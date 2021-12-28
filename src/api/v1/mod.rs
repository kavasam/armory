/*
 * Copyright (C) 2021  Aravinth Manivannan <realaravinth@batsense.net>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of the
 * License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <https://www.gnu.org/licenses/>.
 */
use actix_web::{web, HttpResponse, Responder};
use libkavasam::ReportMessage;

//use crate::errors::*;
use crate::AppData;

pub mod routes {
    pub struct Routes {
        pub report: &'static str,
    }

    impl Routes {
        pub const fn new() -> Self {
            Self {
                report: "/api/v1/report",
            }
        }
    }
}

pub const ROUTES: routes::Routes = routes::Routes::new();

#[my_codegen::post(path = "ROUTES.report")]
pub async fn report(payload: web::Json<ReportMessage>, data: AppData) -> impl Responder {
    if !payload.verify() {
        todo!("return bad request")
    }

    let pkey = payload.public_key.asci_armor();

    sqlx::query!(
        "
        INSERT INTO kavasam_users (public_key) 
        VALUES ($1) 
        ON CONFLICT(public_key) 
            DO NOTHING",
        &pkey
    )
    .execute(&data.db)
    .await
    .unwrap();

    let id_type = serde_json::to_string(&payload.id_type).unwrap();

    for hash in payload.hashes.iter() {
        let hash = hash.ascii_armor().hash;
        sqlx::query!(
            "INSERT INTO kavasam_hashes (hash, id_type) 
                   VALUES ($1, $2);",
            &hash,
            &id_type
        )
        .execute(&data.db)
        .await
        .unwrap();

        sqlx::query!(
            "INSERT INTO kavasam_reports (hash_id, reported_by) 
             VALUES (
               (SELECT ID from kavasam_hashes WHERE id_type = $1 AND hash = $2),
               (SELECT ID from kavasam_users WHERE public_key = $3)
              );",
            &id_type,
            &hash,
            &pkey
        )
        .execute(&data.db)
        .await
        .unwrap();
    }

    HttpResponse::Ok()
}

pub fn services(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(report);
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::*;
    use libkavasam::id::Identity;
    use libkavasam::IDType;

    #[macro_export]
    macro_rules! get_app {
        () => {
            actix_web::test::init_service(
                App::new()
                    .wrap(get_identity_service())
                    .wrap(actix_middleware::NormalizePath::new(
                        actix_middleware::TrailingSlash::Trim,
                    ))
                    .configure(services),
            )
        };
        ($data:expr) => {
            actix_web::test::init_service(
                App::new()
                    .wrap(actix_middleware::NormalizePath::new(
                        actix_middleware::TrailingSlash::Trim,
                    ))
                    .configure(services)
                    .app_data(actix_web::web::Data::new($data.clone())),
            )
        };
    }

    #[macro_export]
    macro_rules! post_request {
        ($uri:expr) => {
            actix_web::test::TestRequest::post().uri($uri)
        };

        ($serializable:expr, $uri:expr) => {
            actix_web::test::TestRequest::post()
                .uri($uri)
                .set_json($serializable)
        };
    }

    async fn delelte_user(id: &Identity, data: &Data) {
        let pkey = id.pub_key().asci_armor();
        let _ = sqlx::query!("DELETE FROM kavasam_users WHERE public_key = ($1)", &pkey)
            .execute(&data.db)
            .await;
    }

    async fn delete_hashes(msg: &ReportMessage, data: &Data) {
        for hash in msg.hashes.iter() {
            let hash = hash.ascii_armor().hash;
            let _ = sqlx::query!("DELETE FROM kavasam_hashes WHERE hash = ($1)", &hash)
                .execute(&data.db)
                .await;
        }
    }

    #[actix_rt::test]
    async fn test_report() {
        const ID: &[u8] = b"foo@example.com";
        let id = Identity::new();
        let msg = ReportMessage::new(ID, IDType::Email, &id);

        let data = Data::new().await;
        delete_hashes(&msg, &data).await;

        let app = get_app!(data).await;

        let add_token_resp = actix_web::test::call_service(
            &app,
            post_request!(&msg, ROUTES.report).to_request(),
        )
        .await;
        assert_eq!(add_token_resp.status(), StatusCode::OK);
        delelte_user(&id, &data).await;
        delete_hashes(&msg, &data).await;
    }
}
