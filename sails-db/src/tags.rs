use crate::{
    error::{SailsDbError, SailsDbResult as Result},
    products::ProductId,
    schema::{tagmappings, tags},
};
use diesel::{dsl::count, prelude::*, sqlite::Sqlite};
use once_cell::sync::Lazy;
use rocket::FromForm;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use uuid::Uuid;

// Built-in tags are the tags that we use throughout the codebase of sails-db and sails-bin.
pub static BUILTIN_TAGS: Lazy<HashMap<Arc<str>, Value>> = Lazy::new(|| {
    use maplit::hashmap;
    hashmap! {
        "digicon".into() => Value {name: "数字内容".to_string(), html: Some(r#"<span class="badge bg-primary"><i class="bi bi-file-earmark-arrow-down-fill"></i> 数字内容</span>"#.to_string()), description: Some("商品包含可访问的数字内容，购买后无需等待即可使用。".to_string()) },
        "ads".into() => Value {name: "广告".to_string(), html: None, description: None },
        "sales".into() => Value {name: "特别优惠".to_string(), html: Some(r#"<span class="badge bg-success"><i class="bi bi-percent"></i> 特别优惠</span>"#.to_string()), description: Some("商品现已加入特别优惠，即刻超值入手".to_string()) },
    }
});

// A pseudo struct for managing the tags table.
pub struct Tags;

impl Tags {
    pub fn list_all(conn: &SqliteConnection) -> Result<Vec<Tag>> {
        use crate::schema::tags::dsl::*;
        Ok(tags.load::<Tag>(conn)?)
    }

    pub fn find_by_id(conn: &SqliteConnection, id_provided: &str) -> Result<Tag> {
        use crate::schema::tags::dsl::*;
        Ok(tags
            .into_boxed()
            .filter(id.eq(id_provided))
            .first::<Tag>(conn)?)
    }

    /// Note: this returns the first category matching the name.
    /// Name is NOT guaranteed to be unique. Whenever possible, use find_by_id instead.
    pub fn find_by_name(conn: &SqliteConnection, name_provided: &str) -> Result<Tag> {
        use crate::schema::tags::dsl::*;
        Ok(tags
            .into_boxed()
            .filter(name.eq(name_provided))
            .first::<Tag>(conn)?)
    }

    // We intentionally don't clean up tagmapping because tags are created in configuration files.
    // Cleaning up the tagmapping renders tags useless as during every startup tags are deleted and recreated.
    pub fn delete_all(conn: &SqliteConnection) -> Result<usize> {
        use crate::schema::tags::dsl::*;
        Ok(diesel::delete(tags).execute(conn)?)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Value {
    pub name: String,
    pub html: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TagsBuilder {
    #[serde(rename = "tags")]
    inner: HashMap<Arc<str>, Value>,
}

impl TagsBuilder {
    pub fn new(inner: HashMap<Arc<str>, Value>) -> Self {
        Self { inner }
    }

    pub fn build(self, conn: &SqliteConnection) -> Result<()> {
        let mut tags = BUILTIN_TAGS.clone();
        // Extend WILL update the existing entries.
        // We would be happy to allow user to override builtins if they need to.
        tags.extend(self.inner.into_iter());
        for (id, value) in tags {
            Tag::create(conn, id, value.name, value.html, value.description)?;
        }
        Ok(())
    }
}

#[derive(
    Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, AsChangeset, Clone, FromForm,
)]
#[table_name = "tags"]
pub struct Tag {
    id: String,
    name: String,
    html: Option<String>,
    description: Option<String>,
}

impl Tag {
    pub fn new(
        id: impl ToString,
        name: impl ToString,
        html: Option<impl ToString>,
        description: Option<impl ToString>,
    ) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            html: html.map(|x| x.to_string()),
            description: description.map(|x| x.to_string()),
        }
    }

    // Create a new tag with a specific ID
    pub fn create(
        conn: &SqliteConnection,
        id_provided: impl ToString,
        name_provided: impl ToString,
        html_provided: Option<impl ToString>,
        description_provided: Option<impl ToString>,
    ) -> Result<Self> {
        use crate::schema::tags::dsl::*;
        let tag = Tag::new(
            id_provided,
            name_provided,
            html_provided,
            description_provided,
        );

        if let Ok(0) = tags.filter(id.eq(tag.get_id())).count().get_result(conn) {
            // This means that we have to insert
            diesel::insert_into(tags).values(&tag).execute(conn)?
        } else {
            return Err(SailsDbError::TagExisted);
        };
        Ok(tag)
    }

    pub fn delete(self, conn: &SqliteConnection) -> Result<usize> {
        use crate::schema::tags::dsl::*;
        Ok(diesel::delete(tags.filter(id.eq(self.id))).execute(conn)?)
    }

    pub fn update(self, conn: &SqliteConnection) -> Result<Self> {
        Ok(self.save_changes::<Tag>(conn)?)
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_description(&self) -> Option<&str> {
        self.description.as_deref()
    }

    pub fn get_html(&self) -> Option<&str> {
        self.html.as_deref()
    }

    pub fn set_name(mut self, name: impl ToString) -> Self {
        self.name = name.to_string();
        self
    }

    pub fn set_description(mut self, description: impl ToString) -> Self {
        self.description = Some(description.to_string());
        self
    }

    pub fn set_html(mut self, html: impl ToString) -> Self {
        self.html = Some(html.to_string());
        self
    }
}

type BoxedQuery<'a> = tagmappings::BoxedQuery<'a, Sqlite, tagmappings::SqlType>;

/// A search query helper (builder)
pub struct TagMappingFinder<'a> {
    conn: &'a SqliteConnection,
    query: BoxedQuery<'a>,
}

impl<'a> TagMappingFinder<'a> {
    pub fn list(conn: &'a SqliteConnection) -> Result<Vec<TagMapping>> {
        Self::new(conn, None).search()
    }

    pub fn search(self) -> Result<Vec<TagMapping>> {
        Ok(self.query.load::<TagMapping>(self.conn)?)
    }

    pub fn search_tag(self) -> Result<Vec<Tag>> {
        let conn = self.conn;
        self.query
            .load::<TagMapping>(conn)?
            .into_iter()
            .map(|x| Tags::find_by_id(conn, x.get_tag()))
            .collect()
    }

    pub fn first(self) -> Result<TagMapping> {
        Ok(self.query.first::<TagMapping>(self.conn)?)
    }

    pub fn delete_by_product(self, product_id: &'a ProductId) -> Result<()> {
        use crate::schema::tagmappings::dsl::*;
        diesel::delete(tagmappings.filter(product.eq(product_id.get_id()))).execute(self.conn)?;
        Ok(())
    }

    pub fn id(mut self, id_provided: &'a str) -> Self {
        use crate::schema::tagmappings::dsl::*;
        self.query = self.query.filter(id.eq(id_provided));
        self
    }

    pub fn product(mut self, product_id: &'a ProductId) -> Self {
        use crate::schema::tagmappings::dsl::*;
        self.query = self.query.filter(product.eq(product_id.get_id()));
        self
    }

    pub fn tag(mut self, tag_id: &'a Tag) -> Self {
        use crate::schema::tagmappings::dsl::*;
        self.query = self.query.filter(tag.eq(tag_id.get_id()));
        self
    }

    pub fn count(self) -> Result<i64> {
        use crate::schema::tagmappings::dsl::*;
        Ok(self.query.select(count(id)).first::<i64>(self.conn)?)
    }

    pub fn has_mapping(
        conn: &'a SqliteConnection,
        tag: &'a Tag,
        product: &'a ProductId,
    ) -> Result<bool> {
        Ok(Self::new(conn, None).tag(tag).product(product).count()? > 0)
    }

    pub fn new(conn: &'a SqliteConnection, query: Option<BoxedQuery<'a>>) -> Self {
        use crate::schema::tagmappings::dsl::*;
        if let Some(q) = query {
            Self { conn, query: q }
        } else {
            Self {
                conn,
                query: tagmappings.into_boxed(),
            }
        }
    }
}

#[derive(
    Debug, Serialize, Deserialize, Queryable, Identifiable, Insertable, AsChangeset, Clone, FromForm,
)]
#[table_name = "tagmappings"]
pub struct TagMapping {
    id: String,
    tag: String,
    product: String,
}

impl TagMapping {
    pub fn create(conn: &SqliteConnection, tag_p: &Tag, product_p: &ProductId) -> Result<Self> {
        // Only create tag mapping if we have not done so.
        if !TagMappingFinder::has_mapping(conn, tag_p, product_p)? {
            use crate::schema::tagmappings::dsl::*;
            let tagmapping = Self {
                id: Uuid::new_v4().to_string(),
                tag: tag_p.get_id().to_string(),
                product: product_p.get_id().to_string(),
            };
            diesel::insert_into(tagmappings)
                .values(&tagmapping)
                .execute(conn)?;
            // There should be one mapping now
            assert!(TagMappingFinder::has_mapping(conn, tag_p, product_p)?);
            Ok(tagmapping)
        } else {
            Err(SailsDbError::TagMappingExisted)
        }
    }

    pub fn get_id(&self) -> &str {
        &self.id
    }

    pub fn get_tag(&self) -> &str {
        &self.tag
    }

    pub fn get_product(&self) -> &str {
        &self.product
    }

    pub fn delete(self, conn: &SqliteConnection) -> Result<usize> {
        use crate::schema::tagmappings::dsl::*;
        Ok(diesel::delete(tagmappings.filter(id.eq(self.id))).execute(conn)?)
    }

    pub fn update(self, conn: &SqliteConnection) -> Result<Self> {
        Ok(self.save_changes::<TagMapping>(conn)?)
    }
}

#[cfg(test)]
mod tests;
