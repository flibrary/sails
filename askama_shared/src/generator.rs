use super::{get_template_source, CompileError, Integrations};
use crate::{
    filters,
    heritage::{Context, Heritage},
    input::{Source, TemplateInput},
    parser::{
        parse, Cond, Expr, MatchParameter, MatchParameters, MatchVariant, Node, Target, When, Ws,
    },
};

use proc_macro2::Span;

use quote::{quote, ToTokens};

use std::{cmp, collections::HashMap, hash, mem, path::PathBuf, str};

pub fn generate<S: std::hash::BuildHasher>(
    input: &TemplateInput,
    contexts: &HashMap<&PathBuf, Context, S>,
    heritage: &Option<Heritage>,
    integrations: Integrations,
) -> Result<String, CompileError> {
    Generator::new(input, contexts, heritage, integrations, MapChain::new())
        .build(&contexts[&input.path])
}

struct Generator<'a, S: std::hash::BuildHasher> {
    // The template input state: original struct AST and attributes
    input: &'a TemplateInput<'a>,
    // All contexts, keyed by the package-relative template path
    contexts: &'a HashMap<&'a PathBuf, Context<'a>, S>,
    // The heritage contains references to blocks and their ancestry
    heritage: &'a Option<Heritage<'a>>,
    // What integrations need to be generated
    integrations: Integrations,
    // Variables accessible directly from the current scope (not redirected to context)
    locals: MapChain<'a, &'a str, LocalMeta>,
    // Suffix whitespace from the previous literal. Will be flushed to the
    // output buffer unless suppressed by whitespace suppression on the next
    // non-literal.
    next_ws: Option<&'a str>,
    // Whitespace suppression from the previous non-literal. Will be used to
    // determine whether to flush prefix whitespace from the next literal.
    skip_ws: bool,
    // If currently in a block, this will contain the name of a potential parent block
    super_block: Option<(&'a str, usize)>,
    // buffer for writable
    buf_writable: Vec<Writable<'a>>,
    // Counter for write! hash named arguments
    named: usize,
}

impl<'a, S: std::hash::BuildHasher> Generator<'a, S> {
    fn new<'n>(
        input: &'n TemplateInput,
        contexts: &'n HashMap<&'n PathBuf, Context<'n>, S>,
        heritage: &'n Option<Heritage>,
        integrations: Integrations,
        locals: MapChain<'n, &'n str, LocalMeta>,
    ) -> Generator<'n, S> {
        Generator {
            input,
            contexts,
            heritage,
            integrations,
            locals,
            next_ws: None,
            skip_ws: false,
            super_block: None,
            buf_writable: vec![],
            named: 0,
        }
    }

    fn child(&mut self) -> Generator<'_, S> {
        let locals = MapChain::with_parent(&self.locals);
        Self::new(
            self.input,
            self.contexts,
            self.heritage,
            self.integrations,
            locals,
        )
    }

    // Takes a Context and generates the relevant implementations.
    fn build(mut self, ctx: &'a Context) -> Result<String, CompileError> {
        let mut buf = Buffer::new(0);
        if !ctx.blocks.is_empty() {
            if let Some(parent) = self.input.parent {
                self.deref_to_parent(&mut buf, parent)?;
            }
        };

        self.impl_template(ctx, &mut buf)?;
        self.impl_display(&mut buf)?;

        if self.integrations.actix {
            self.impl_actix_web_responder(&mut buf)?;
        }
        if self.integrations.gotham {
            self.impl_gotham_into_response(&mut buf)?;
        }
        if self.integrations.iron {
            self.impl_iron_modifier_response(&mut buf)?;
        }
        if self.integrations.mendes {
            self.impl_mendes_responder(&mut buf)?;
        }
        if self.integrations.rocket {
            self.impl_rocket_responder(&mut buf)?;
        }
        if self.integrations.tide {
            self.impl_tide_integrations(&mut buf)?;
        }
        if self.integrations.warp {
            self.impl_warp_reply(&mut buf)?;
        }
        Ok(buf.buf)
    }

    // Implement `Template` for the given context struct.
    fn impl_template(&mut self, ctx: &'a Context, buf: &mut Buffer) -> Result<(), CompileError> {
        self.write_header(buf, "::askama::Template", None)?;
        buf.writeln(
            "fn render_into(&self, writer: &mut dyn ::std::fmt::Write) -> \
             ::askama::Result<()> {",
        )?;

        // Make sure the compiler understands that the generated code depends on the template files.
        for path in self.contexts.keys() {
            // Skip the fake path of templates defined in rust source.
            let path_is_valid = match self.input.source {
                Source::Path(_) => true,
                Source::Source(_) => *path != &self.input.path,
            };
            if path_is_valid {
                let path = path.to_str().unwrap();
                buf.writeln(
                    &quote! {
                        include_bytes!(#path);
                    }
                    .to_string(),
                )?;
            }
        }

        let size_hint = if let Some(heritage) = self.heritage {
            self.handle(heritage.root, heritage.root.nodes, buf, AstLevel::Top)
        } else {
            self.handle(ctx, &ctx.nodes, buf, AstLevel::Top)
        }?;

        self.flush_ws(Ws(false, false));
        buf.writeln("Ok(())")?;
        buf.writeln("}")?;

        buf.writeln("fn extension(&self) -> Option<&'static str> {")?;
        buf.writeln(&format!("{:?}", self.input.extension()))?;
        buf.writeln("}")?;

        buf.writeln("fn size_hint(&self) -> usize {")?;
        buf.writeln(&format!("{}", size_hint))?;
        buf.writeln("}")?;

        buf.writeln("}")?;

        self.write_header(buf, "::askama::SizedTemplate", None)?;

        buf.writeln("fn size_hint() -> usize {")?;
        buf.writeln(&format!("{}", size_hint))?;
        buf.writeln("}")?;

        buf.writeln("fn extension() -> Option<&'static str> {")?;
        buf.writeln(&format!("{:?}", self.input.extension()))?;
        buf.writeln("}")?;

        buf.writeln("}")?;
        Ok(())
    }

    // Implement `Deref<Parent>` for an inheriting context struct.
    fn deref_to_parent(
        &mut self,
        buf: &mut Buffer,
        parent_type: &syn::Type,
    ) -> Result<(), CompileError> {
        self.write_header(buf, "::std::ops::Deref", None)?;
        buf.writeln(&format!(
            "type Target = {};",
            parent_type.into_token_stream()
        ))?;
        buf.writeln("fn deref(&self) -> &Self::Target {")?;
        buf.writeln("&self._parent")?;
        buf.writeln("}")?;
        buf.writeln("}")
    }

    // Implement `Display` for the given context struct.
    fn impl_display(&mut self, buf: &mut Buffer) -> Result<(), CompileError> {
        self.write_header(buf, "::std::fmt::Display", None)?;
        buf.writeln("fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {")?;
        buf.writeln("::askama::Template::render_into(self, f).map_err(|_| ::std::fmt::Error {})")?;
        buf.writeln("}")?;
        buf.writeln("}")
    }

    // Implement Actix-web's `Responder`.
    fn impl_actix_web_responder(&mut self, buf: &mut Buffer) -> Result<(), CompileError> {
        self.write_header(buf, "::actix_web::Responder", None)?;
        buf.writeln("type Future = ::askama_actix::futures::Ready<::std::result::Result<::actix_web::HttpResponse, Self::Error>>;")?;
        buf.writeln("type Error = ::actix_web::Error;")?;
        buf.writeln(
            "fn respond_to(self, _req: &::actix_web::HttpRequest) \
             -> Self::Future {",
        )?;

        buf.writeln("use ::askama_actix::TemplateToResponse;")?;
        buf.writeln("::askama_actix::futures::ready(self.to_response())")?;

        buf.writeln("}")?;
        buf.writeln("}")
    }

    // Implement gotham's `IntoResponse`.
    fn impl_gotham_into_response(&mut self, buf: &mut Buffer) -> Result<(), CompileError> {
        self.write_header(buf, "::askama_gotham::IntoResponse", None)?;
        buf.writeln(
            "fn into_response(self, _state: &::askama_gotham::State)\
             -> ::askama_gotham::Response<::askama_gotham::Body> {",
        )?;
        let ext = self.input.extension().unwrap_or("txt");
        buf.writeln(&format!("::askama_gotham::respond(&self, {:?})", ext))?;
        buf.writeln("}")?;
        buf.writeln("}")
    }

    // Implement iron's Modifier<Response> if enabled
    fn impl_iron_modifier_response(&mut self, buf: &mut Buffer) -> Result<(), CompileError> {
        self.write_header(
            buf,
            "::askama_iron::Modifier<::askama_iron::Response>",
            None,
        )?;
        buf.writeln("fn modify(self, res: &mut ::askama_iron::Response) {")?;
        buf.writeln(
            "res.body = Some(Box::new(::askama_iron::Template::render(&self).unwrap().into_bytes()));",
        )?;

        match self.input.extension().unwrap_or("") {
            "html" | "htm" => {
                buf.writeln("::askama_iron::ContentType::html().0.modify(res);")?;
            }
            _ => (),
        };

        buf.writeln("}")?;
        buf.writeln("}")
    }

    // Implement mendes' `Responder`.
    fn impl_mendes_responder(&mut self, buf: &mut Buffer) -> Result<(), CompileError> {
        let param = syn::parse_str("A: ::mendes::Application").unwrap();

        let mut generics = self.input.ast.generics.clone();
        generics.params.push(param);
        let (_, orig_ty_generics, _) = self.input.ast.generics.split_for_impl();
        let (impl_generics, _, where_clause) = generics.split_for_impl();

        let mut where_clause = match where_clause {
            Some(clause) => clause.clone(),
            None => syn::WhereClause {
                where_token: syn::Token![where](Span::call_site()),
                predicates: syn::punctuated::Punctuated::new(),
            },
        };

        where_clause
            .predicates
            .push(syn::parse_str("A::ResponseBody: From<String>").unwrap());
        where_clause
            .predicates
            .push(syn::parse_str("A::Error: From<::mendes::askama::Error>").unwrap());

        buf.writeln(
            format!(
                "{} {} for {} {} {{",
                quote!(impl#impl_generics),
                "::mendes::application::Responder<A>",
                self.input.ast.ident,
                quote!(#orig_ty_generics #where_clause),
            )
            .as_ref(),
        )?;

        buf.writeln(
            "fn into_response(self, app: &A, req: &::mendes::http::request::Parts) \
             -> ::mendes::http::Response<A::ResponseBody> {",
        )?;

        buf.writeln(&format!(
            "::mendes::askama::into_response(app, req, &self, {:?})",
            self.input.extension()
        ))?;
        buf.writeln("}")?;
        buf.writeln("}")?;
        Ok(())
    }

    // Implement Rocket's `Responder`.
    fn impl_rocket_responder(&mut self, buf: &mut Buffer) -> Result<(), CompileError> {
        let req_lifetime = syn::Lifetime::new("'askama", Span::call_site());
        let param0 = syn::GenericParam::Lifetime(syn::LifetimeDef::new(req_lifetime.clone()));
        let out_lifetime = syn::Lifetime::new("'out", Span::call_site());
        let mut out_def = syn::LifetimeDef::new(out_lifetime);
        out_def.bounds.push(req_lifetime);
        let param1 = syn::GenericParam::Lifetime(out_def);

        self.write_header(
            buf,
            "::askama_rocket::Responder<'askama, 'out>",
            Some(vec![param0, param1]),
        )?;
        buf.writeln(
            "fn respond_to(self, _: &::askama_rocket::Request) \
             -> ::askama_rocket::Result<'out> {",
        )?;
        let ext = match self.input.path.extension() {
            Some(s) => s.to_str().unwrap(),
            None => "txt",
        };
        buf.writeln(&format!("::askama_rocket::respond(&self, {:?})", ext))?;
        buf.writeln("}")?;
        buf.writeln("}")?;
        Ok(())
    }

    fn impl_tide_integrations(&mut self, buf: &mut Buffer) -> Result<(), CompileError> {
        let ext = self.input.extension().unwrap_or("txt");

        self.write_header(
            buf,
            "std::convert::TryInto<::askama_tide::tide::Body>",
            None,
        )?;
        buf.writeln(
            "type Error = ::askama_tide::askama::Error;\n\
            fn try_into(self) -> ::askama_tide::askama::Result<::askama_tide::tide::Body> {",
        )?;
        buf.writeln(&format!("::askama_tide::try_into_body(&self, {:?})", &ext))?;
        buf.writeln("}")?;
        buf.writeln("}")?;

        buf.writeln("#[allow(clippy::from_over_into)]")?;
        self.write_header(buf, "Into<::askama_tide::tide::Response>", None)?;
        buf.writeln("fn into(self) -> ::askama_tide::tide::Response {")?;
        buf.writeln(&format!("::askama_tide::into_response(&self, {:?})", ext))?;
        buf.writeln("}\n}")
    }

    fn impl_warp_reply(&mut self, buf: &mut Buffer) -> Result<(), CompileError> {
        self.write_header(buf, "::askama_warp::warp::reply::Reply", None)?;
        buf.writeln("fn into_response(self) -> ::askama_warp::warp::reply::Response {")?;
        let ext = self.input.extension().unwrap_or("txt");
        buf.writeln(&format!("::askama_warp::reply(&self, {:?})", ext))?;
        buf.writeln("}")?;
        buf.writeln("}")
    }

    // Writes header for the `impl` for `TraitFromPathName` or `Template`
    // for the given context struct.
    fn write_header(
        &mut self,
        buf: &mut Buffer,
        target: &str,
        params: Option<Vec<syn::GenericParam>>,
    ) -> Result<(), CompileError> {
        let mut generics = self.input.ast.generics.clone();
        if let Some(params) = params {
            for param in params {
                generics.params.push(param);
            }
        }
        let (_, orig_ty_generics, _) = self.input.ast.generics.split_for_impl();
        let (impl_generics, _, where_clause) = generics.split_for_impl();
        buf.writeln(
            format!(
                "{} {} for {}{} {{",
                quote!(impl#impl_generics),
                target,
                self.input.ast.ident,
                quote!(#orig_ty_generics #where_clause),
            )
            .as_ref(),
        )
    }

    /* Helper methods for handling node types */

    fn handle(
        &mut self,
        ctx: &'a Context,
        nodes: &'a [Node],
        buf: &mut Buffer,
        level: AstLevel,
    ) -> Result<usize, CompileError> {
        let mut size_hint = 0;
        for n in nodes {
            match *n {
                Node::Lit(lws, val, rws) => {
                    self.visit_lit(lws, val, rws);
                }
                Node::Comment(ws) => {
                    self.write_comment(ws);
                }
                Node::Expr(ws, ref val) => {
                    self.write_expr(ws, val);
                }
                Node::LetDecl(ws, ref var) => {
                    self.write_let_decl(buf, ws, var)?;
                }
                Node::Let(ws, ref var, ref val) => {
                    self.write_let(buf, ws, var, val)?;
                }
                Node::Cond(ref conds, ws) => {
                    self.write_cond(ctx, buf, conds, ws)?;
                }
                Node::Match(ws1, ref expr, ref arms, ws2) => {
                    self.write_match(ctx, buf, ws1, expr, arms, ws2)?;
                }
                Node::Loop(ws1, ref var, ref iter, ref body, ws2) => {
                    self.write_loop(ctx, buf, ws1, var, iter, body, ws2)?;
                }
                Node::BlockDef(ws1, name, _, ws2) => {
                    self.write_block(buf, Some(name), Ws(ws1.0, ws2.1))?;
                }
                Node::Include(ws, path) => {
                    size_hint += self.handle_include(ctx, buf, ws, path)?;
                }
                Node::Call(ws, scope, name, ref args) => {
                    size_hint += self.write_call(ctx, buf, ws, scope, name, args)?;
                }
                Node::Macro(_, ref m) => {
                    if level != AstLevel::Top {
                        return Err("macro blocks only allowed at the top level".into());
                    }
                    self.flush_ws(m.ws1);
                    self.prepare_ws(m.ws2);
                }
                Node::Raw(ws1, contents, ws2) => {
                    self.handle_ws(ws1);
                    self.buf_writable.push(Writable::Lit(contents));
                    self.handle_ws(ws2);
                }
                Node::Import(ws, _, _) => {
                    if level != AstLevel::Top {
                        return Err("import blocks only allowed at the top level".into());
                    }
                    self.handle_ws(ws);
                }
                Node::Extends(_) => {
                    if level != AstLevel::Top {
                        return Err("extend blocks only allowed at the top level".into());
                    }
                    // No whitespace handling: child template top-level is not used,
                    // except for the blocks defined in it.
                }
            }
        }

        if AstLevel::Top == level {
            size_hint += self.write_buf_writable(buf)?;
        }
        Ok(size_hint)
    }

    fn write_cond(
        &mut self,
        ctx: &'a Context,
        buf: &mut Buffer,
        conds: &'a [Cond],
        ws: Ws,
    ) -> Result<usize, CompileError> {
        let mut flushed = 0;
        let mut arm_sizes = Vec::new();
        let mut has_else = false;
        for (i, &(cws, ref cond, ref nodes)) in conds.iter().enumerate() {
            self.handle_ws(cws);
            flushed += self.write_buf_writable(buf)?;
            if i > 0 {
                self.locals.pop();
            }

            let mut arm_size = 0;
            match *cond {
                Some(ref expr) => {
                    if i == 0 {
                        buf.write("if ");
                    } else {
                        buf.dedent()?;
                        buf.write("} else if ");
                    }
                    // The following syntax `*(&(...) as &bool)` is used to
                    // trigger Rust's automatic dereferencing, to coerce
                    // e.g. `&&&&&bool` to `bool`. First `&(...) as &bool`
                    // coerces e.g. `&&&bool` to `&bool`. Then `*(&bool)`
                    // finally dereferences it to `bool`.
                    buf.write("*(&(");
                    let expr_code = self.visit_expr_root(expr)?;
                    buf.write(&expr_code);
                    buf.write(") as &bool)");
                }
                None => {
                    buf.dedent()?;
                    buf.write("} else");
                    has_else = true;
                }
            }

            buf.writeln(" {")?;
            self.locals.push();

            arm_size += self.handle(ctx, nodes, buf, AstLevel::Nested)?;
            arm_sizes.push(arm_size);
        }
        self.handle_ws(ws);
        flushed += self.write_buf_writable(buf)?;
        buf.writeln("}")?;

        self.locals.pop();

        if !has_else {
            arm_sizes.push(0);
        }
        Ok(flushed + median(&mut arm_sizes))
    }

    #[allow(clippy::too_many_arguments)]
    fn write_match(
        &mut self,
        ctx: &'a Context,
        buf: &mut Buffer,
        ws1: Ws,
        expr: &Expr,
        arms: &'a [When],
        ws2: Ws,
    ) -> Result<usize, CompileError> {
        self.flush_ws(ws1);
        let flushed = self.write_buf_writable(buf)?;
        let mut arm_sizes = Vec::new();

        let expr_code = self.visit_expr_root(expr)?;
        buf.writeln(&format!("match &{} {{", expr_code))?;

        let mut arm_size = 0;
        for (i, arm) in arms.iter().enumerate() {
            let &(ws, ref variant, ref params, ref body) = arm;
            self.handle_ws(ws);

            if i > 0 {
                arm_sizes.push(arm_size + self.write_buf_writable(buf)?);

                buf.writeln("}")?;
                self.locals.pop();
            }

            self.locals.push();
            match *variant {
                Some(ref param) => {
                    self.visit_match_variant(buf, param);
                }
                None => buf.write("_"),
            };

            match params {
                MatchParameters::Simple(params) => {
                    if !params.is_empty() {
                        buf.write("(");
                        for (i, param) in params.iter().enumerate() {
                            if let MatchParameter::Name(p) = *param {
                                self.locals.insert_with_default(p);
                            }
                            if i > 0 {
                                buf.write(", ");
                            }
                            self.visit_match_param(buf, param);
                        }
                        buf.write(")");
                    }
                }
                MatchParameters::Named(params) => {
                    buf.write("{");
                    for (i, param) in params.iter().enumerate() {
                        if let Some(MatchParameter::Name(p)) = param.1 {
                            let p = normalize_identifier(p);
                            self.locals.insert_with_default(p);
                        } else {
                            self.locals.insert_with_default(param.0);
                        }

                        if i > 0 {
                            buf.write(", ");
                        }
                        buf.write(param.0);
                        if let Some(param) = &param.1 {
                            buf.write(":");
                            self.visit_match_param(buf, &param);
                        }
                    }
                    buf.write("}");
                }
            }
            buf.writeln(" => {")?;

            arm_size = self.handle(ctx, body, buf, AstLevel::Nested)?;
        }

        self.handle_ws(ws2);
        arm_sizes.push(arm_size + self.write_buf_writable(buf)?);
        buf.writeln("}")?;
        self.locals.pop();

        buf.writeln("}")?;

        Ok(flushed + median(&mut arm_sizes))
    }

    #[allow(clippy::too_many_arguments)]
    fn write_loop(
        &mut self,
        ctx: &'a Context,
        buf: &mut Buffer,
        ws1: Ws,
        var: &'a Target,
        iter: &Expr,
        body: &'a [Node],
        ws2: Ws,
    ) -> Result<usize, CompileError> {
        self.handle_ws(ws1);
        self.locals.push();

        let expr_code = self.visit_expr_root(iter)?;

        let flushed = self.write_buf_writable(buf)?;
        buf.write("for (");
        self.visit_target(buf, var);
        match iter {
            Expr::Range(_, _, _) => buf.writeln(&format!(
                ", _loop_item) in ::askama::helpers::TemplateLoop::new({}) {{",
                expr_code
            )),
            Expr::Array(..) => buf.writeln(&format!(
                ", _loop_item) in ::askama::helpers::TemplateLoop::new({}.iter()) {{",
                expr_code
            )),
            // If `iter` is a call then we assume it's something that returns
            // an iterator. If not then the user can explicitly add the needed
            // call without issues.
            Expr::MethodCall(..) | Expr::PathCall(..) | Expr::Index(..) => buf.writeln(&format!(
                ", _loop_item) in ::askama::helpers::TemplateLoop::new(({}).into_iter()) {{",
                expr_code
            )),
            // If accessing `self` then it most likely needs to be
            // borrowed, to prevent an attempt of moving.
            _ if expr_code.starts_with("self.") => buf.writeln(&format!(
                ", _loop_item) in ::askama::helpers::TemplateLoop::new(((&{}).into_iter())) {{",
                expr_code
            )),
            // Otherwise, we borrow `iter` assuming that it implements `IntoIterator`.
            _ => buf.writeln(&format!(
                ", _loop_item) in ::askama::helpers::TemplateLoop::new(({}).into_iter()) {{",
                expr_code
            )),
        }?;

        let mut size_hint = self.handle(ctx, body, buf, AstLevel::Nested)?;
        self.handle_ws(ws2);

        size_hint += self.write_buf_writable(buf)?;
        buf.writeln("}")?;
        self.locals.pop();
        Ok(flushed + (size_hint * 3))
    }

    fn write_call(
        &mut self,
        ctx: &'a Context,
        buf: &mut Buffer,
        ws: Ws,
        scope: Option<&str>,
        name: &str,
        args: &[Expr],
    ) -> Result<usize, CompileError> {
        if name == "super" {
            return self.write_block(buf, None, ws);
        }

        let (def, own_ctx) = if let Some(s) = scope {
            let path = ctx.imports.get(s).ok_or_else(|| {
                CompileError::String(format!("no import found for scope '{}'", s))
            })?;
            let mctx = self.contexts.get(path).ok_or_else(|| {
                CompileError::String(format!("context for '{:?}' not found", path))
            })?;
            (
                mctx.macros.get(name).ok_or_else(|| {
                    CompileError::String(format!("macro '{}' not found in scope '{}'", name, s))
                })?,
                mctx,
            )
        } else {
            (
                ctx.macros
                    .get(name)
                    .ok_or_else(|| CompileError::String(format!("macro '{}' not found", name)))?,
                ctx,
            )
        };

        self.flush_ws(ws); // Cannot handle_ws() here: whitespace from macro definition comes first
        self.locals.push();
        self.write_buf_writable(buf)?;
        buf.writeln("{")?;
        self.prepare_ws(def.ws1);

        let mut names = Buffer::new(0);
        let mut values = Buffer::new(0);
        for (i, arg) in def.args.iter().enumerate() {
            let expr = args.get(i).ok_or_else(|| {
                CompileError::String(format!("macro '{}' takes more than {} arguments", name, i))
            })?;

            match expr {
                // If `expr` is already a form of variable then
                // don't reintroduce a new variable. This is
                // to avoid moving non-copyable values.
                Expr::Var(name) => {
                    let var = self.locals.resolve_or_self(name);
                    self.locals.insert(arg, LocalMeta::with_ref(var));
                }
                Expr::Attr(obj, attr) => {
                    let mut attr_buf = Buffer::new(0);
                    self.visit_attr(&mut attr_buf, obj, attr)?;

                    let var = self.locals.resolve(&attr_buf.buf).unwrap_or(attr_buf.buf);
                    self.locals.insert(arg, LocalMeta::with_ref(var));
                }
                // Everything else still needs to become variables,
                // to avoid having the same logic be executed
                // multiple times, e.g. in the case of macro
                // parameters being used multiple times.
                _ => {
                    if i > 0 {
                        names.write(", ");
                        values.write(", ");
                    }
                    names.write(arg);

                    values.write("(");
                    values.write(&self.visit_expr_root(expr)?);
                    values.write(")");
                    self.locals.insert_with_default(arg);
                }
            }
        }

        debug_assert_eq!(names.buf.is_empty(), values.buf.is_empty());
        if !names.buf.is_empty() {
            buf.writeln(&format!("let ({}) = ({});", names.buf, values.buf))?;
        }

        let mut size_hint = self.handle(own_ctx, &def.nodes, buf, AstLevel::Nested)?;

        self.flush_ws(def.ws2);
        size_hint += self.write_buf_writable(buf)?;
        buf.writeln("}")?;
        self.locals.pop();
        self.prepare_ws(ws);
        Ok(size_hint)
    }

    fn handle_include(
        &mut self,
        ctx: &'a Context,
        buf: &mut Buffer,
        ws: Ws,
        path: &str,
    ) -> Result<usize, CompileError> {
        self.flush_ws(ws);
        self.write_buf_writable(buf)?;
        let path = self
            .input
            .config
            .find_template(path, Some(&self.input.path))?;
        let src = get_template_source(&path)?;
        let nodes = parse(&src, self.input.syntax)?;

        // Make sure the compiler understands that the generated code depends on the template file.
        {
            let path = path.to_str().unwrap();
            buf.writeln(
                &quote! {
                    include_bytes!(#path);
                }
                .to_string(),
            )?;
        }

        let size_hint = {
            // Since nodes must not outlive the Generator, we instantiate
            // a nested Generator here to handle the include's nodes.
            let mut gen = self.child();
            let mut size_hint = gen.handle(ctx, &nodes, buf, AstLevel::Nested)?;
            size_hint += gen.write_buf_writable(buf)?;
            size_hint
        };
        self.prepare_ws(ws);
        Ok(size_hint)
    }

    fn write_let_decl(
        &mut self,
        buf: &mut Buffer,
        ws: Ws,
        var: &'a Target,
    ) -> Result<(), CompileError> {
        self.handle_ws(ws);
        self.write_buf_writable(buf)?;
        buf.write("let ");
        match *var {
            Target::Name(name) => {
                let name = normalize_identifier(name);
                self.locals.insert_with_default(name);
                buf.write(name);
            }
            Target::Tuple(ref targets) => {
                buf.write("(");
                for name in targets {
                    let name = normalize_identifier(name);
                    self.locals.insert_with_default(name);
                    buf.write(name);
                    buf.write(",");
                }
                buf.write(")");
            }
        }
        buf.writeln(";")
    }

    fn write_let(
        &mut self,
        buf: &mut Buffer,
        ws: Ws,
        var: &'a Target,
        val: &Expr,
    ) -> Result<(), CompileError> {
        self.handle_ws(ws);
        let mut expr_buf = Buffer::new(0);
        self.visit_expr(&mut expr_buf, val)?;

        match *var {
            Target::Name(name) => {
                let name = normalize_identifier(name);
                let meta = self.locals.get(&name).cloned();

                let shadowed = matches!(&meta, Some(meta) if meta.initialized);
                if shadowed {
                    // Need to flush the buffer if the variable is being shadowed,
                    // to ensure the old variable is used.
                    self.write_buf_writable(buf)?;
                }
                if shadowed || meta.is_none() {
                    buf.write("let ");
                }
                buf.write(name);

                self.locals.insert(name, LocalMeta::initialized());
            }
            Target::Tuple(ref targets) => {
                let shadowed = targets.iter().any(|name| {
                    let name = normalize_identifier(name);
                    matches!(self.locals.get(&name), Some(meta) if meta.initialized)
                });
                if shadowed {
                    // Need to flush the buffer if the variable is being shadowed,
                    // to ensure the old variable is used.
                    self.write_buf_writable(buf)?;
                }

                buf.write("let (");
                for name in targets {
                    let name = normalize_identifier(name);
                    self.locals.insert(name, LocalMeta::initialized());
                    buf.write(name);
                    buf.write(",");
                }
                buf.write(")");
            }
        }
        buf.writeln(&format!(" = {};", &expr_buf.buf))
    }

    // If `name` is `Some`, this is a call to a block definition, and we have to find
    // the first block for that name from the ancestry chain. If name is `None`, this
    // is from a `super()` call, and we can get the name from `self.super_block`.
    fn write_block(
        &mut self,
        buf: &mut Buffer,
        name: Option<&'a str>,
        outer: Ws,
    ) -> Result<usize, CompileError> {
        // Flush preceding whitespace according to the outer WS spec
        self.flush_ws(outer);

        let prev_block = self.super_block;
        let cur = match (name, prev_block) {
            // The top-level context contains a block definition
            (Some(cur_name), None) => (cur_name, 0),
            // A block definition contains a block definition of the same name
            (Some(cur_name), Some((prev_name, _))) if cur_name == prev_name => {
                return Err(format!("cannot define recursive blocks ({})", cur_name).into());
            }
            // A block definition contains a definition of another block
            (Some(cur_name), Some((_, _))) => (cur_name, 0),
            // `super()` was called inside a block
            (None, Some((prev_name, gen))) => (prev_name, gen + 1),
            // `super()` is called from outside a block
            (None, None) => return Err("cannot call 'super()' outside block".into()),
        };
        self.super_block = Some(cur);

        // Get the block definition from the heritage chain
        let heritage = self
            .heritage
            .as_ref()
            .ok_or(CompileError::Static("no block ancestors available"))?;
        let (ctx, def) = heritage.blocks[cur.0].get(cur.1).ok_or_else(|| {
            CompileError::from(match name {
                None => format!("no super() block found for block '{}'", cur.0),
                Some(name) => format!("no block found for name '{}'", name),
            })
        })?;

        // Get the nodes and whitespace suppression data from the block definition
        let (ws1, nodes, ws2) = if let Node::BlockDef(ws1, _, nodes, ws2) = def {
            (ws1, nodes, ws2)
        } else {
            unreachable!()
        };

        // Handle inner whitespace suppression spec and process block nodes
        self.prepare_ws(*ws1);
        self.locals.push();
        let size_hint = self.handle(ctx, nodes, buf, AstLevel::Block)?;

        if !self.locals.is_current_empty() {
            // Need to flush the buffer before popping the variable stack
            self.write_buf_writable(buf)?;
        }

        self.locals.pop();
        self.flush_ws(*ws2);

        // Restore original block context and set whitespace suppression for
        // succeeding whitespace according to the outer WS spec
        self.super_block = prev_block;
        self.prepare_ws(outer);
        Ok(size_hint)
    }

    fn write_expr(&mut self, ws: Ws, s: &'a Expr<'a>) {
        self.handle_ws(ws);
        self.buf_writable.push(Writable::Expr(s));
    }

    // Write expression buffer and empty
    fn write_buf_writable(&mut self, buf: &mut Buffer) -> Result<usize, CompileError> {
        if self.buf_writable.is_empty() {
            return Ok(0);
        }

        if self
            .buf_writable
            .iter()
            .all(|w| matches!(w, Writable::Lit(_)))
        {
            let mut buf_lit = Buffer::new(0);
            for s in mem::take(&mut self.buf_writable) {
                if let Writable::Lit(s) = s {
                    buf_lit.write(s);
                };
            }
            buf.writeln(&format!("writer.write_str({:#?})?;", &buf_lit.buf))?;
            return Ok(buf_lit.buf.len());
        }

        let mut size_hint = 0;
        let mut buf_format = Buffer::new(0);
        let mut buf_expr = Buffer::new(buf.indent + 1);
        let mut expr_cache = HashMap::with_capacity(self.buf_writable.len());
        for s in mem::take(&mut self.buf_writable) {
            match s {
                Writable::Lit(s) => {
                    buf_format.write(&s.replace("{", "{{").replace("}", "}}"));
                    size_hint += s.len();
                }
                Writable::Expr(s) => {
                    use self::DisplayWrap::*;
                    let mut expr_buf = Buffer::new(0);
                    let wrapped = self.visit_expr(&mut expr_buf, s)?;
                    let expression = match wrapped {
                        Wrapped => expr_buf.buf,
                        Unwrapped => format!(
                            "::askama::MarkupDisplay::new_unsafe(&({}), {})",
                            expr_buf.buf, self.input.escaper
                        ),
                    };

                    use std::collections::hash_map::Entry;
                    let id = match expr_cache.entry(expression.clone()) {
                        Entry::Occupied(e) => *e.get(),
                        Entry::Vacant(e) => {
                            let id = self.named;
                            self.named += 1;

                            buf_expr.write(&format!("expr{} = ", id));
                            buf_expr.write("&");
                            buf_expr.write(&expression);
                            buf_expr.writeln(",")?;

                            e.insert(id);
                            id
                        }
                    };

                    buf_format.write(&format!("{{expr{}}}", id));
                    size_hint += 3;
                }
            }
        }

        buf.writeln("write!(")?;
        buf.indent();
        buf.writeln("writer,")?;
        buf.writeln(&format!("{:#?},", &buf_format.buf))?;
        buf.writeln(buf_expr.buf.trim())?;
        buf.dedent()?;
        buf.writeln(")?;")?;
        Ok(size_hint)
    }

    fn visit_lit(&mut self, lws: &'a str, val: &'a str, rws: &'a str) {
        assert!(self.next_ws.is_none());
        if !lws.is_empty() {
            if self.skip_ws {
                self.skip_ws = false;
            } else if val.is_empty() {
                assert!(rws.is_empty());
                self.next_ws = Some(lws);
            } else {
                self.buf_writable.push(Writable::Lit(lws));
            }
        }

        if !val.is_empty() {
            self.buf_writable.push(Writable::Lit(val));
        }

        if !rws.is_empty() {
            self.next_ws = Some(rws);
        }
    }

    fn write_comment(&mut self, ws: Ws) {
        self.handle_ws(ws);
    }

    /* Visitor methods for expression types */

    fn visit_expr_root(&mut self, expr: &Expr) -> Result<String, CompileError> {
        let mut buf = Buffer::new(0);
        self.visit_expr(&mut buf, expr)?;
        Ok(buf.buf)
    }

    fn visit_expr(&mut self, buf: &mut Buffer, expr: &Expr) -> Result<DisplayWrap, CompileError> {
        Ok(match *expr {
            Expr::BoolLit(s) => self.visit_bool_lit(buf, s),
            Expr::NumLit(s) => self.visit_num_lit(buf, s),
            Expr::StrLit(s) => self.visit_str_lit(buf, s),
            Expr::CharLit(s) => self.visit_char_lit(buf, s),
            Expr::Var(s) => self.visit_var(buf, s),
            Expr::VarCall(var, ref args) => self.visit_var_call(buf, var, args)?,
            Expr::Path(ref path) => self.visit_path(buf, path),
            Expr::PathCall(ref path, ref args) => self.visit_path_call(buf, path, args)?,
            Expr::Array(ref elements) => self.visit_array(buf, elements)?,
            Expr::Attr(ref obj, name) => self.visit_attr(buf, obj, name)?,
            Expr::Index(ref obj, ref key) => self.visit_index(buf, obj, key)?,
            Expr::Filter(name, ref args) => self.visit_filter(buf, name, args)?,
            Expr::Unary(op, ref inner) => self.visit_unary(buf, op, inner)?,
            Expr::BinOp(op, ref left, ref right) => self.visit_binop(buf, op, left, right)?,
            Expr::Range(op, ref left, ref right) => self.visit_range(buf, op, left, right)?,
            Expr::Group(ref inner) => self.visit_group(buf, inner)?,
            Expr::MethodCall(ref obj, method, ref args) => {
                self.visit_method_call(buf, obj, method, args)?
            }
            Expr::RustMacro(name, args) => self.visit_rust_macro(buf, name, args),
        })
    }

    fn visit_rust_macro(&mut self, buf: &mut Buffer, name: &str, args: &str) -> DisplayWrap {
        buf.write(name);
        buf.write("!(");
        buf.write(args);
        buf.write(")");

        DisplayWrap::Unwrapped
    }

    fn visit_match_variant(&mut self, buf: &mut Buffer, param: &MatchVariant) -> DisplayWrap {
        let mut expr_buf = Buffer::new(0);
        let wrapped = match *param {
            MatchVariant::StrLit(s) => {
                expr_buf.write("&");
                self.visit_str_lit(&mut expr_buf, s)
            }
            MatchVariant::CharLit(s) => self.visit_char_lit(&mut expr_buf, s),
            MatchVariant::NumLit(s) => self.visit_num_lit(&mut expr_buf, s),
            MatchVariant::Name(s) => {
                expr_buf.write(s);
                DisplayWrap::Unwrapped
            }
            MatchVariant::Path(ref s) => {
                expr_buf.write(&s.join("::"));
                DisplayWrap::Unwrapped
            }
        };
        buf.write(&expr_buf.buf);
        wrapped
    }

    fn visit_match_param(&mut self, buf: &mut Buffer, param: &MatchParameter) -> DisplayWrap {
        let mut expr_buf = Buffer::new(0);
        let wrapped = match *param {
            MatchParameter::NumLit(s) => self.visit_num_lit(&mut expr_buf, s),
            MatchParameter::StrLit(s) => self.visit_str_lit(&mut expr_buf, s),
            MatchParameter::CharLit(s) => self.visit_char_lit(&mut expr_buf, s),
            MatchParameter::Name(s) => {
                expr_buf.write(s);
                DisplayWrap::Unwrapped
            }
        };
        buf.write(&expr_buf.buf);
        wrapped
    }

    fn visit_filter(
        &mut self,
        buf: &mut Buffer,
        mut name: &str,
        args: &[Expr],
    ) -> Result<DisplayWrap, CompileError> {
        if name == "format" {
            self._visit_format_filter(buf, args)?;
            return Ok(DisplayWrap::Unwrapped);
        } else if name == "fmt" {
            self._visit_fmt_filter(buf, args)?;
            return Ok(DisplayWrap::Unwrapped);
        } else if name == "join" {
            self._visit_join_filter(buf, args)?;
            return Ok(DisplayWrap::Unwrapped);
        }

        if name == "tojson" {
            name = "json";
        }

        #[cfg(not(feature = "json"))]
        if name == "json" {
            return Err("the `json` filter requires the `serde-json` feature to be enabled".into());
        }
        #[cfg(not(feature = "yaml"))]
        if name == "yaml" {
            return Err("the `yaml` filter requires the `serde-yaml` feature to be enabled".into());
        }

        const FILTERS: [&str; 5] = ["safe", "escape", "e", "json", "yaml"];
        if FILTERS.contains(&name) {
            buf.write(&format!(
                "::askama::filters::{}({}, ",
                name, self.input.escaper
            ));
        } else if filters::BUILT_IN_FILTERS.contains(&name) {
            buf.write(&format!("::askama::filters::{}(", name));
        } else {
            buf.write(&format!("filters::{}(", name));
        }

        self._visit_args(buf, args)?;
        buf.write(")?");
        Ok(match FILTERS.contains(&name) {
            true => DisplayWrap::Wrapped,
            false => DisplayWrap::Unwrapped,
        })
    }

    fn _visit_format_filter(
        &mut self,
        buf: &mut Buffer,
        args: &[Expr],
    ) -> Result<(), CompileError> {
        buf.write("format!(");
        if let Some(Expr::StrLit(v)) = args.first() {
            self.visit_str_lit(buf, v);
            if args.len() > 1 {
                buf.write(", ");
            }
        } else {
            return Err("invalid expression type for format filter".into());
        }
        self._visit_args(buf, &args[1..])?;
        buf.write(")");
        Ok(())
    }

    fn _visit_fmt_filter(&mut self, buf: &mut Buffer, args: &[Expr]) -> Result<(), CompileError> {
        buf.write("format!(");
        if let Some(Expr::StrLit(v)) = args.get(1) {
            self.visit_str_lit(buf, v);
            buf.write(", ");
        } else {
            return Err("invalid expression type for fmt filter".into());
        }
        self._visit_args(buf, &args[0..1])?;
        if args.len() > 2 {
            return Err("only two arguments allowed to fmt filter".into());
        }
        buf.write(")");
        Ok(())
    }

    // Force type coercion on first argument to `join` filter (see #39).
    fn _visit_join_filter(&mut self, buf: &mut Buffer, args: &[Expr]) -> Result<(), CompileError> {
        buf.write("::askama::filters::join((&");
        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                buf.write(", &");
            }
            self.visit_expr(buf, arg)?;
            if i == 0 {
                buf.write(").into_iter()");
            }
        }
        buf.write(")?");
        Ok(())
    }

    fn _visit_args(&mut self, buf: &mut Buffer, args: &[Expr]) -> Result<(), CompileError> {
        if args.is_empty() {
            return Ok(());
        }

        for (i, arg) in args.iter().enumerate() {
            if i > 0 {
                buf.write(", ");
            }

            let borrow = !arg.is_copyable();
            if borrow {
                buf.write("&(");
            }

            let scoped = matches!(
                arg,
                Expr::Filter(_, _)
                    | Expr::MethodCall(_, _, _)
                    | Expr::VarCall(_, _)
                    | Expr::PathCall(_, _)
            );

            if scoped {
                buf.writeln("{")?;
                self.visit_expr(buf, arg)?;
                buf.writeln("}")?;
            } else {
                self.visit_expr(buf, arg)?;
            }

            if borrow {
                buf.write(")");
            }
        }
        Ok(())
    }

    fn visit_attr(
        &mut self,
        buf: &mut Buffer,
        obj: &Expr,
        attr: &str,
    ) -> Result<DisplayWrap, CompileError> {
        if let Expr::Var(name) = *obj {
            if name == "loop" {
                if attr == "index" {
                    buf.write("(_loop_item.index + 1)");
                    return Ok(DisplayWrap::Unwrapped);
                } else if attr == "index0" {
                    buf.write("_loop_item.index");
                    return Ok(DisplayWrap::Unwrapped);
                } else if attr == "first" {
                    buf.write("_loop_item.first");
                    return Ok(DisplayWrap::Unwrapped);
                } else if attr == "last" {
                    buf.write("_loop_item.last");
                    return Ok(DisplayWrap::Unwrapped);
                } else {
                    return Err("unknown loop variable".into());
                }
            }
        }
        self.visit_expr(buf, obj)?;
        buf.write(&format!(".{}", attr));
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_index(
        &mut self,
        buf: &mut Buffer,
        obj: &Expr,
        key: &Expr,
    ) -> Result<DisplayWrap, CompileError> {
        buf.write("&");
        self.visit_expr(buf, obj)?;
        buf.write("[");
        self.visit_expr(buf, key)?;
        buf.write("]");
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_method_call(
        &mut self,
        buf: &mut Buffer,
        obj: &Expr,
        method: &str,
        args: &[Expr],
    ) -> Result<DisplayWrap, CompileError> {
        if let Expr::Var("self") = obj {
            buf.write("self");
        } else {
            self.visit_expr(buf, obj)?;
        }

        buf.write(&format!(".{}(", method));
        self._visit_args(buf, args)?;
        buf.write(")");
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_unary(
        &mut self,
        buf: &mut Buffer,
        op: &str,
        inner: &Expr,
    ) -> Result<DisplayWrap, CompileError> {
        buf.write(op);
        self.visit_expr(buf, inner)?;
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_range(
        &mut self,
        buf: &mut Buffer,
        op: &str,
        left: &Option<Box<Expr>>,
        right: &Option<Box<Expr>>,
    ) -> Result<DisplayWrap, CompileError> {
        if let Some(left) = left {
            self.visit_expr(buf, left)?;
        }
        buf.write(op);
        if let Some(right) = right {
            self.visit_expr(buf, right)?;
        }
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_binop(
        &mut self,
        buf: &mut Buffer,
        op: &str,
        left: &Expr,
        right: &Expr,
    ) -> Result<DisplayWrap, CompileError> {
        self.visit_expr(buf, left)?;
        buf.write(&format!(" {} ", op));
        self.visit_expr(buf, right)?;
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_group(&mut self, buf: &mut Buffer, inner: &Expr) -> Result<DisplayWrap, CompileError> {
        buf.write("(");
        self.visit_expr(buf, inner)?;
        buf.write(")");
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_array(
        &mut self,
        buf: &mut Buffer,
        elements: &[Expr],
    ) -> Result<DisplayWrap, CompileError> {
        buf.write("[");
        for (i, el) in elements.iter().enumerate() {
            if i > 0 {
                buf.write(", ");
            }
            self.visit_expr(buf, el)?;
        }
        buf.write("]");
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_path(&mut self, buf: &mut Buffer, path: &[&str]) -> DisplayWrap {
        for (i, part) in path.iter().enumerate() {
            if i > 0 {
                buf.write("::");
            }
            buf.write(part);
        }
        DisplayWrap::Unwrapped
    }

    fn visit_path_call(
        &mut self,
        buf: &mut Buffer,
        path: &[&str],
        args: &[Expr],
    ) -> Result<DisplayWrap, CompileError> {
        for (i, part) in path.iter().enumerate() {
            if i > 0 {
                buf.write("::");
            }
            buf.write(part);
        }
        buf.write("(");
        self._visit_args(buf, args)?;
        buf.write(")");
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_var(&mut self, buf: &mut Buffer, s: &str) -> DisplayWrap {
        if s == "self" {
            buf.write(s);
            return DisplayWrap::Unwrapped;
        }

        buf.write(normalize_identifier(&self.locals.resolve_or_self(&s)));
        DisplayWrap::Unwrapped
    }

    fn visit_var_call(
        &mut self,
        buf: &mut Buffer,
        s: &str,
        args: &[Expr],
    ) -> Result<DisplayWrap, CompileError> {
        buf.write("(");
        let s = normalize_identifier(s);
        if !self.locals.contains(&s) && s != "self" {
            buf.write("self.");
        }
        buf.write(s);
        buf.write(")(");
        self._visit_args(buf, args)?;
        buf.write(")");
        Ok(DisplayWrap::Unwrapped)
    }

    fn visit_bool_lit(&mut self, buf: &mut Buffer, s: &str) -> DisplayWrap {
        buf.write(s);
        DisplayWrap::Unwrapped
    }

    fn visit_str_lit(&mut self, buf: &mut Buffer, s: &str) -> DisplayWrap {
        buf.write(&format!("\"{}\"", s));
        DisplayWrap::Unwrapped
    }

    fn visit_char_lit(&mut self, buf: &mut Buffer, s: &str) -> DisplayWrap {
        buf.write(&format!("'{}'", s));
        DisplayWrap::Unwrapped
    }

    fn visit_num_lit(&mut self, buf: &mut Buffer, s: &str) -> DisplayWrap {
        buf.write(s);
        DisplayWrap::Unwrapped
    }

    fn visit_target(&mut self, buf: &mut Buffer, target: &'a Target) {
        match *target {
            Target::Name(name) => {
                let name = normalize_identifier(name);
                self.locals.insert_with_default(name);
                buf.write(name);
            }
            Target::Tuple(ref targets) => {
                buf.write("(");
                for name in targets {
                    let name = normalize_identifier(name);
                    self.locals.insert_with_default(name);
                    buf.write(name);
                    buf.write(",");
                }
                buf.write(")");
            }
        }
    }

    /* Helper methods for dealing with whitespace nodes */

    // Combines `flush_ws()` and `prepare_ws()` to handle both trailing whitespace from the
    // preceding literal and leading whitespace from the succeeding literal.
    fn handle_ws(&mut self, ws: Ws) {
        self.flush_ws(ws);
        self.prepare_ws(ws);
    }

    // If the previous literal left some trailing whitespace in `next_ws` and the
    // prefix whitespace suppressor from the given argument, flush that whitespace.
    // In either case, `next_ws` is reset to `None` (no trailing whitespace).
    fn flush_ws(&mut self, ws: Ws) {
        if self.next_ws.is_some() && !ws.0 {
            let val = self.next_ws.unwrap();
            if !val.is_empty() {
                self.buf_writable.push(Writable::Lit(val));
            }
        }
        self.next_ws = None;
    }

    // Sets `skip_ws` to match the suffix whitespace suppressor from the given
    // argument, to determine whether to suppress leading whitespace from the
    // next literal.
    fn prepare_ws(&mut self, ws: Ws) {
        self.skip_ws = ws.1;
    }
}

struct Buffer {
    // The buffer to generate the code into
    buf: String,
    // The current level of indentation (in spaces)
    indent: u8,
    // Whether the output buffer is currently at the start of a line
    start: bool,
}

impl Buffer {
    fn new(indent: u8) -> Self {
        Self {
            buf: String::new(),
            indent,
            start: true,
        }
    }

    fn writeln(&mut self, s: &str) -> Result<(), CompileError> {
        if s == "}" {
            self.dedent()?;
        }
        if !s.is_empty() {
            self.write(s);
        }
        self.buf.push('\n');
        if s.ends_with('{') {
            self.indent();
        }
        self.start = true;
        Ok(())
    }

    fn write(&mut self, s: &str) {
        if self.start {
            for _ in 0..(self.indent * 4) {
                self.buf.push(' ');
            }
            self.start = false;
        }
        self.buf.push_str(s);
    }

    fn indent(&mut self) {
        self.indent += 1;
    }

    fn dedent(&mut self) -> Result<(), CompileError> {
        if self.indent == 0 {
            return Err("dedent() called while indentation == 0".into());
        }
        self.indent -= 1;
        Ok(())
    }
}

#[derive(Clone, Default)]
struct LocalMeta {
    refs: Option<String>,
    initialized: bool,
}

impl LocalMeta {
    fn initialized() -> Self {
        Self {
            refs: None,
            initialized: true,
        }
    }

    fn with_ref(refs: String) -> Self {
        Self {
            refs: Some(refs),
            initialized: true,
        }
    }
}

// type SetChain<'a, T> = MapChain<'a, T, ()>;

#[derive(Debug)]
struct MapChain<'a, K: 'a, V: 'a>
where
    K: cmp::Eq + hash::Hash,
{
    parent: Option<&'a MapChain<'a, K, V>>,
    scopes: Vec<HashMap<K, V>>,
}

impl<'a, K: 'a, V: 'a> MapChain<'a, K, V>
where
    K: cmp::Eq + hash::Hash,
{
    fn new() -> MapChain<'a, K, V> {
        MapChain {
            parent: None,
            scopes: vec![HashMap::new()],
        }
    }

    fn with_parent<'p>(parent: &'p MapChain<K, V>) -> MapChain<'p, K, V> {
        MapChain {
            parent: Some(parent),
            scopes: vec![HashMap::new()],
        }
    }

    fn contains(&self, key: &K) -> bool {
        self.scopes.iter().rev().any(|set| set.contains_key(&key))
            || match self.parent {
                Some(set) => set.contains(key),
                None => false,
            }
    }

    /// Iterates the scopes in reverse and returns `Some(LocalMeta)`
    /// from the first scope where `key` exists.
    fn get(&self, key: &K) -> Option<&V> {
        let scopes = self.scopes.iter().rev();
        scopes
            .filter_map(|set| set.get(&key))
            .next()
            .or_else(|| self.parent.and_then(|set| set.get(key)))
    }

    fn is_current_empty(&self) -> bool {
        self.scopes.last().unwrap().is_empty()
    }

    fn insert(&mut self, key: K, val: V) {
        self.scopes.last_mut().unwrap().insert(key, val);

        // Note that if `insert` returns `Some` then it implies
        // an identifier is reused. For e.g. `{% macro f(a, a) %}`
        // and `{% let (a, a) = ... %}` then this results in a
        // generated template, which when compiled fails with the
        // compile error "identifier `a` used more than once".
    }

    fn insert_with_default(&mut self, key: K)
    where
        V: Default,
    {
        self.insert(key, V::default());
    }

    fn push(&mut self) {
        self.scopes.push(HashMap::new());
    }

    fn pop(&mut self) {
        self.scopes.pop().unwrap();
        assert!(!self.scopes.is_empty());
    }
}

impl MapChain<'_, &str, LocalMeta> {
    fn resolve(&self, name: &str) -> Option<String> {
        let name = normalize_identifier(name);
        self.get(&name).map(|meta| match &meta.refs {
            Some(expr) => expr.clone(),
            None => name.to_string(),
        })
    }

    fn resolve_or_self(&self, name: &str) -> String {
        let name = normalize_identifier(name);
        self.resolve(name)
            .unwrap_or_else(|| format!("self.{}", name))
    }
}

fn median(sizes: &mut [usize]) -> usize {
    sizes.sort_unstable();
    if sizes.len() % 2 == 1 {
        sizes[sizes.len() / 2]
    } else {
        (sizes[sizes.len() / 2 - 1] + sizes[sizes.len() / 2]) / 2
    }
}

#[derive(Clone, PartialEq)]
enum AstLevel {
    Top,
    Block,
    Nested,
}

impl Copy for AstLevel {}

#[derive(Clone)]
enum DisplayWrap {
    Wrapped,
    Unwrapped,
}

impl Copy for DisplayWrap {}

#[derive(Debug)]
enum Writable<'a> {
    Lit(&'a str),
    Expr(&'a Expr<'a>),
}

// Identifiers to be replaced with raw identifiers, so as to avoid
// collisions between template syntax and Rust's syntax. In particular
// [Rust keywords](https://doc.rust-lang.org/reference/keywords.html)
// should be replaced, since they're not reserved words in Askama
// syntax but have a high probability of causing problems in the
// generated code.
//
// This list excludes the Rust keywords *self*, *Self*, and *super*
// because they are not allowed to be raw identifiers, and *loop*
// because it's used something like a keyword in the template
// language.
static USE_RAW: [(&str, &str); 47] = [
    ("as", "r#as"),
    ("break", "r#break"),
    ("const", "r#const"),
    ("continue", "r#continue"),
    ("crate", "r#crate"),
    ("else", "r#else"),
    ("enum", "r#enum"),
    ("extern", "r#extern"),
    ("false", "r#false"),
    ("fn", "r#fn"),
    ("for", "r#for"),
    ("if", "r#if"),
    ("impl", "r#impl"),
    ("in", "r#in"),
    ("let", "r#let"),
    ("match", "r#match"),
    ("mod", "r#mod"),
    ("move", "r#move"),
    ("mut", "r#mut"),
    ("pub", "r#pub"),
    ("ref", "r#ref"),
    ("return", "r#return"),
    ("static", "r#static"),
    ("struct", "r#struct"),
    ("trait", "r#trait"),
    ("true", "r#true"),
    ("type", "r#type"),
    ("unsafe", "r#unsafe"),
    ("use", "r#use"),
    ("where", "r#where"),
    ("while", "r#while"),
    ("async", "r#async"),
    ("await", "r#await"),
    ("dyn", "r#dyn"),
    ("abstract", "r#abstract"),
    ("become", "r#become"),
    ("box", "r#box"),
    ("do", "r#do"),
    ("final", "r#final"),
    ("macro", "r#macro"),
    ("override", "r#override"),
    ("priv", "r#priv"),
    ("typeof", "r#typeof"),
    ("unsized", "r#unsized"),
    ("virtual", "r#virtual"),
    ("yield", "r#yield"),
    ("try", "r#try"),
];

fn normalize_identifier(ident: &str) -> &str {
    if let Some(word) = USE_RAW.iter().find(|x| x.0 == ident) {
        word.1
    } else {
        ident
    }
}
