use crate::ctx::Ctx;
use crate::model::ModelManager;
use crate::model::{Error, Result};
use lib_base::time::now_utc;
use sea_query::{DynIden, Expr, Iden, IntoIden, Order, PostgresQueryBuilder, Query};
use sea_query_binder::SqlxBinder;
use sqlb::{Field, Fields, HasFields, SIden};
use sqlx::postgres::PgRow;
use sqlx::FromRow;

#[derive(Iden)]
pub enum CommonSpec {
	Id,
}

#[derive(Iden)]
pub enum TimestampSpec {
	Cid,
	Ctime,
	Mid,
	Mtime,
}

pub trait DbBmc {
	const TABLE: &'static str;
	const TIMESTAMP: bool;

	fn table_dyn() -> DynIden {
		SIden(Self::TABLE).into_iden()
	}
}

pub async fn create<MC, E>(ctx: &Ctx, mm: &ModelManager, data: E) -> Result<i64>
where
	MC: DbBmc,
	E: HasFields,
{
	let db = mm.db();

	// -- Build query
	let mut fields = data.not_none_fields();
	add_timestamp(&mut fields, ctx.user_id(), true, true);
	let (columns, values) = fields.unzip();

	let (sql, values) = Query::insert()
		.into_table(MC::table_dyn())
		.columns(columns)
		.values(values)?
		.returning(Query::returning().columns([CommonSpec::Id]))
		.build_sqlx(PostgresQueryBuilder);

	// -- Exec query
	let (id,) = sqlx::query_as_with::<_, (i64,), _>(&sql, values)
		.fetch_one(db)
		.await?;

	Ok(id)
}

pub async fn get<MC, E>(_ctx: &Ctx, mm: &ModelManager, id: i64) -> Result<E>
where
	MC: DbBmc,
	E: for<'r> FromRow<'r, PgRow> + Unpin + Send,
	E: HasFields,
{
	let db = mm.db();

	// -- Build query
	let (sql, values) = Query::select()
		.from(MC::table_dyn())
		.columns(E::field_idens())
		.and_where(Expr::col(CommonSpec::Id).eq(id))
		.build_sqlx(PostgresQueryBuilder);

	// -- Exec query
	let entity = sqlx::query_as_with::<_, E, _>(&sql, values)
		.fetch_optional(db)
		.await?
		.ok_or(Error::EntityNotFound {
			entity: MC::TABLE,
			id,
		})?;

	Ok(entity)
}

pub async fn list<MC, E>(_ctx: &Ctx, mm: &ModelManager) -> Result<Vec<E>>
where
	MC: DbBmc,
	E: for<'r> FromRow<'r, PgRow> + Unpin + Send,
	E: HasFields,
{
	let db = mm.db();

	// -- Build query
	let (sql, values) = Query::select()
		.from(MC::table_dyn())
		.columns(E::field_idens())
		.order_by(CommonSpec::Id, Order::Asc)
		.build_sqlx(PostgresQueryBuilder);

	// -- Execute the query
	let entities = sqlx::query_as_with::<_, E, _>(&sql, values)
		.fetch_all(db)
		.await?;

	Ok(entities)
}

pub async fn update<MC, E>(
	ctx: &Ctx,
	mm: &ModelManager,
	id: i64,
	data: E,
) -> Result<()>
where
	MC: DbBmc,
	E: HasFields,
{
	let db = mm.db();

	// -- Build query
	let mut fields = data.not_none_fields();
	add_timestamp(&mut fields, ctx.user_id(), false, true);
	let fields = fields.zip();

	let (sql, values) = Query::update()
		.table(MC::table_dyn())
		.values(fields)
		.and_where(Expr::col(CommonSpec::Id).eq(id))
		.build_sqlx(PostgresQueryBuilder);

	// -- Execute query
	let count = sqlx::query_with(&sql, values)
		.execute(db)
		.await?
		.rows_affected();

	// -- Check result
	if count == 0 {
		Err(Error::EntityNotFound {
			entity: MC::TABLE,
			id,
		})
	} else {
		Ok(())
	}
}

pub async fn delete<MC>(_ctx: &Ctx, mm: &ModelManager, id: i64) -> Result<()>
where
	MC: DbBmc,
{
	let db = mm.db();

	let (sql, values) = Query::delete()
		.from_table(MC::table_dyn())
		.and_where(Expr::col(CommonSpec::Id).eq(id))
		.build_sqlx(PostgresQueryBuilder);

	let count = sqlx::query_with(&sql, values)
		.execute(db)
		.await?
		.rows_affected();

	// -- Check result
	if count == 0 {
		Err(Error::EntityNotFound {
			entity: MC::TABLE,
			id,
		})
	} else {
		Ok(())
	}
}

// region:    --- Utils

fn add_timestamp(fields: &mut Fields, user_id: i64, create: bool, update: bool) {
	let now = now_utc();
	if create {
		fields.push(Field::new(TimestampSpec::Cid.into_iden(), user_id.into()));
		fields.push(Field::new(TimestampSpec::Ctime.into_iden(), now.into()));
	}

	if update {
		fields.push(Field::new(TimestampSpec::Mid.into_iden(), user_id.into()));
		fields.push(Field::new(TimestampSpec::Mtime.into_iden(), now.into()));
	}
}
// endregion: --- Utils
