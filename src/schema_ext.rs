use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// A wrapper type for DateTime<Utc> to implement the Schema trait
#[derive(Serialize, Deserialize, ToSchema)]
#[schema(value_type = String, format = "date-time", example = "2023-01-01T12:00:00Z")]
pub struct DateTimeWrapper(pub DateTime<Utc>);

/// A wrapper type for UUID to implement the Schema trait
#[derive(Serialize, Deserialize, ToSchema)]
#[schema(value_type = String, format = "uuid", example = "123e4567-e89b-12d3-a456-426614174000")]
pub struct UuidWrapper(pub Uuid);
