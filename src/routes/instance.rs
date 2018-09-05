use gettextrs::gettext;
use rocket::{request::LenientForm, response::Redirect};
use rocket_contrib::{Json, Template};
use serde_json;
use validator::{Validate};

use plume_models::{
    admin::Admin,
    comments::Comment,
    db_conn::DbConn,
    posts::Post,
    users::User,
    instance::*
};
use inbox::Inbox;
use routes::Page;

#[get("/?<page>")]
fn paginated_index(conn: DbConn, user: Option<User>, page: Page) -> Template {
    match Instance::get_local(&*conn) {
        Some(inst) => {
            let recents = Post::get_recents_page(&*conn, page.limits());

            Template::render("instance/index", json!({
                "instance": inst,
                "account": user.map(|u| u.to_json(&*conn)),
                "recents": recents.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>(),
                "page": page.page,
                "n_pages": Page::total(Post::count(&*conn) as i32),
                "n_users": User::count_local(&*conn),
                "n_articles": Post::count_local(&*conn)
            }))
        }
        None => {
            Template::render("errors/500", json!({
                "error_message": gettext("You need to configure your instance before using it.".to_string())
            }))
        }
    }
}

#[get("/")]
fn index(conn: DbConn, user: Option<User>) -> Template {
    paginated_index(conn, user, Page::first())
}

#[get("/local?<page>")]
fn paginated_local(conn: DbConn, user: Option<User>, page: Page) -> Template {
    let instance = Instance::get_local(&*conn).unwrap();
    let articles = Post::get_instance_page(&*conn, instance.id, page.limits());
    Template::render("instance/local", json!({
        "account": user.map(|u| u.to_json(&*conn)),
        "instance": instance,
        "page": page.page,
        "n_pages": Page::total(Post::count_local(&*conn) as i32),
        "articles": articles.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>()
    }))
}

#[get("/local")]
fn local(conn: DbConn, user: Option<User>) -> Template {
    paginated_local(conn, user, Page::first())
}

#[get("/feed")]
fn feed(conn: DbConn, user: User) -> Template {
    paginated_feed(conn, user, Page::first())
}

#[get("/feed?<page>")]
fn paginated_feed(conn: DbConn, user: User, page: Page) -> Template {
    let followed = user.get_following(&*conn);
    let articles = Post::user_feed_page(&*conn, followed.into_iter().map(|u| u.id).collect(), page.limits());
    Template::render("instance/feed", json!({
        "account": user.to_json(&*conn),
        "page": page.page,
        "n_pages": Page::total(Post::count_local(&*conn) as i32),
        "articles": articles.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>()
    }))
}

#[get("/federated")]
fn federated(conn: DbConn, user: Option<User>) -> Template {
    paginated_federated(conn, user, Page::first())
}

#[get("/federated?<page>")]
fn paginated_federated(conn: DbConn, user: Option<User>, page: Page) -> Template {
    let articles = Post::get_recents_page(&*conn, page.limits());
    Template::render("instance/federated", json!({
        "account": user.map(|u| u.to_json(&*conn)),
        "page": page.page,
        "n_pages": Page::total(Post::count_local(&*conn) as i32),
        "articles": articles.into_iter().map(|p| p.to_json(&*conn)).collect::<Vec<serde_json::Value>>()
    }))
}

#[get("/admin")]
fn admin(conn: DbConn, admin: Admin) -> Template {
    Template::render("instance/admin", json!({
        "account": admin.0.to_json(&*conn),
        "instance": Instance::get_local(&*conn),
        "errors": null,
        "form": null
    }))
}

#[derive(FromForm, Validate, Serialize)]
struct InstanceSettingsForm {
    #[validate(length(min = "1"))]
    name: String,
    open_registrations: bool,
    short_description: String,
    long_description: String,
    #[validate(length(min = "1"))]
    default_license: String
}

#[post("/admin", data = "<form>")]
fn update_settings(conn: DbConn, admin: Admin, form: LenientForm<InstanceSettingsForm>) -> Result<Redirect, Template> {
    let form = form.get();
    form.validate()
        .map(|_| {
            let instance = Instance::get_local(&*conn).unwrap();
            instance.update(&*conn,
                form.name.clone(),
                form.open_registrations,
                form.short_description.clone(),
                form.long_description.clone());
            Redirect::to(uri!(admin))
        })
        .map_err(|e| Template::render("instance/admin", json!({
            "account": admin.0.to_json(&*conn),
            "instance": Instance::get_local(&*conn),
            "errors": e.inner(),
            "form": form
        })))
}

#[post("/inbox", data = "<data>")]
fn shared_inbox(conn: DbConn, data: String) -> String {
    let act: serde_json::Value = serde_json::from_str(&data[..]).unwrap();
    let instance = Instance::get_local(&*conn).unwrap();
    match instance.received(&*conn, act) {
        Ok(_) => String::new(),
        Err(e) => {
            println!("Shared inbox error: {}\n{}", e.cause(), e.backtrace());
            format!("Error: {}", e.cause())
        }
    }
}

#[get("/nodeinfo")]
fn nodeinfo(conn: DbConn) -> Json<serde_json::Value> {
    Json(json!({
        "version": "2.0",
        "software": {
            "name": "Plume",
            "version": "0.1.0"
        },
        "protocols": ["activitypub"],
        "services": {
            "inbound": [],
            "outbound": []
        },
        "openRegistrations": true,
        "usage": {
            "users": {
                "total": User::count_local(&*conn)
            },
            "localPosts": Post::count_local(&*conn),
            "localComments": Comment::count_local(&*conn)
        },
        "metadata": {}
    }))
}

#[get("/about")]
fn about(user: Option<User>, conn: DbConn) -> Template {
    Template::render("instance/about", json!({
        "account": user.map(|u| u.to_json(&*conn)),
        "instance": Instance::get_local(&*conn),
        "admin": Instance::get_local(&*conn).map(|i| i.main_admin(&*conn).to_json(&*conn)),
        "version": "0.1.0",
        "n_users": User::count_local(&*conn),
        "n_articles": Post::count_local(&*conn),
        "n_instances": Instance::count(&*conn) - 1
    }))
}
