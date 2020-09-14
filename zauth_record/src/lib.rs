
pub trait Record {
	type Output;
	type Db;

	fn last(db: &Self::Db) -> Result<Self::Output, diesel::result::Error>;
}