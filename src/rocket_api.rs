use rocket::http::{Cookie, Cookies, Status};
use rocket::request::{self, FromRequest, Request};
use rocket::{Outcome, State};
use rocket_contrib::json::Json;
use std::path::PathBuf;
use std::sync::Arc;

use config::{self, Config};
use db::DB;
use errors;
use index;
use user;

const CURRENT_MAJOR_VERSION: i32 = 2;
const CURRENT_MINOR_VERSION: i32 = 2;
const SESSION_FIELD_USERNAME: &str = "username";

pub fn get_routes() -> Vec<rocket::Route> {
	routes![
		version,
		initial_setup,
		get_settings,
		put_settings,
		trigger_index,
		auth,
		browse_root,
		browse,
		flatten_root,
		flatten,
		random,
		recent,
	]
}

struct Auth {
	username: String,
}

impl<'a, 'r> FromRequest<'a, 'r> for Auth {
	type Error = ();

	fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, ()> {
		let mut cookies = request.guard::<Cookies>().unwrap();
		match cookies.get_private(SESSION_FIELD_USERNAME) {
			Some(u) => Outcome::Success(Auth {
				username: u.to_string(),
			}),
			_ => Outcome::Failure((Status::Forbidden, ())),
		}

		// TODO allow auth via authorization header
	}
}

struct AdminRights {}
impl<'a, 'r> FromRequest<'a, 'r> for AdminRights {
	type Error = ();

	fn from_request(request: &'a Request<'r>) -> request::Outcome<Self, ()> {
		let db = request.guard::<State<DB>>()?;

		match user::count::<DB>(&db) {
			Err(_) => return Outcome::Failure((Status::InternalServerError, ())),
			Ok(0) => return Outcome::Success(AdminRights {}),
			_ => (),
		};

		let auth = request.guard::<Auth>()?;
		match user::is_admin::<DB>(&db, &auth.username) {
			Err(_) => Outcome::Failure((Status::InternalServerError, ())),
			Ok(true) => Outcome::Success(AdminRights {}),
			Ok(false) => Outcome::Failure((Status::Forbidden, ())),
		}
	}
}

#[derive(Serialize)]
struct Version {
	major: i32,
	minor: i32,
}

#[get("/version")]
fn version() -> Json<Version> {
	let current_version = Version {
		major: CURRENT_MAJOR_VERSION,
		minor: CURRENT_MINOR_VERSION,
	};
	Json(current_version)
}

#[derive(Serialize)]
struct InitialSetup {
	has_any_users: bool,
}

#[get("/initial_setup")]
fn initial_setup(db: State<DB>) -> Result<Json<InitialSetup>, errors::Error> {
	let initial_setup = InitialSetup {
		has_any_users: user::count::<DB>(&db)? > 0,
	};
	Ok(Json(initial_setup))
}

#[get("/settings")]
fn get_settings(db: State<DB>, _admin_rights: AdminRights) -> Result<Json<Config>, errors::Error> {
	let config = config::read::<DB>(&db)?;
	Ok(Json(config))
}

#[put("/settings", data = "<config>")]
fn put_settings(
	db: State<DB>,
	_admin_rights: AdminRights,
	config: Json<Config>,
) -> Result<(), errors::Error> {
	config::amend::<DB>(&db, &config)?;
	Ok(())
}

#[post("/trigger_index")]
fn trigger_index(
	command_sender: State<Arc<index::CommandSender>>,
	_admin_rights: AdminRights,
) -> Result<(), errors::Error> {
	command_sender.trigger_reindex()?;
	Ok(())
}

#[derive(Deserialize)]
struct AuthCredentials {
	username: String,
	password: String,
}

#[derive(Serialize)]
struct AuthOutput {
	admin: bool,
}

#[post("/auth", data = "<credentials>")]
fn auth(
	db: State<DB>,
	credentials: Json<AuthCredentials>,
	mut cookies: Cookies,
) -> Result<(Json<AuthOutput>), errors::Error> {
	user::auth::<DB>(&db, &credentials.username, &credentials.password)?;
	cookies.add_private(Cookie::new(
		SESSION_FIELD_USERNAME,
		credentials.username.clone(),
	));

	let auth_output = AuthOutput {
		admin: user::is_admin::<DB>(&db, &credentials.username)?,
	};
	Ok(Json(auth_output))
}

#[get("/browse")]
fn browse_root(
	db: State<DB>,
	_auth: Auth,
) -> Result<(Json<Vec<index::CollectionFile>>), errors::Error> {
	let result = index::browse::<DB>(&db, &PathBuf::new())?;
	Ok(Json(result))
}

#[get("/browse/<path..>")]
fn browse(
	db: State<DB>,
	_auth: Auth,
	path: PathBuf,
) -> Result<(Json<Vec<index::CollectionFile>>), errors::Error> {
	let result = index::browse::<DB>(&db, &path)?;
	Ok(Json(result))
}

#[get("/flatten")]
fn flatten_root(db: State<DB>, _auth: Auth) -> Result<(Json<Vec<index::Song>>), errors::Error> {
	let result = index::flatten::<DB>(&db, &PathBuf::new())?;
	Ok(Json(result))
}

#[get("/flatten/<path..>")]
fn flatten(
	db: State<DB>,
	_auth: Auth,
	path: PathBuf,
) -> Result<(Json<Vec<index::Song>>), errors::Error> {
	let result = index::flatten::<DB>(&db, &path)?;
	Ok(Json(result))
}

#[get("/random")]
fn random(db: State<DB>, _auth: Auth) -> Result<(Json<Vec<index::Directory>>), errors::Error> {
	let result = index::get_random_albums::<DB>(&db, 20)?;
	Ok(Json(result))
}

#[get("/recent")]
fn recent(db: State<DB>, _auth: Auth) -> Result<(Json<Vec<index::Directory>>), errors::Error> {
	let result = index::get_recent_albums::<DB>(&db, 20)?;
	Ok(Json(result))
}
