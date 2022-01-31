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
use libkavasam::id::PublicKey;
use libkavasam::IDType;
use libkavasam::ReportMessage;
use serde::{Deserialize, Serialize};

//use crate::errors::*;
use crate::AppData;

pub mod routes {
    pub struct Routes {
        pub report: &'static str,
        pub get_all_reported_by: &'static str,
    }

    impl Routes {
        pub const fn new() -> Self {
            Self {
                report: "/api/v1/report",
                get_all_reported_by: "/api/v1/query/reported_by",
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

    let mut tags = Vec::with_capacity(payload.tags.len());

    for tag in &payload.tags {
        let tag = data.creds.username(tag).unwrap();
        sqlx::query!(
            "
        INSERT INTO kavasam_tags (name)
        VALUES ($1)
        ON CONFLICT(name) DO NOTHING;
        ",
            tag
        )
        .execute(&data.db)
        .await
        .unwrap();
        tags.push(tag);
    }

    for hash in payload.hashes.iter() {
        let hash = hash.ascii_armor().hash;
        sqlx::query!(
            "INSERT INTO kavasam_hashes (hash, id_type) 
             VALUES ($1, $2) 
             ON CONFLICT(hash) DO NOTHING;",
            &hash,
            &id_type
        )
        .execute(&data.db)
        .await
        .unwrap();

        // TODO possible unique constraint violation"
        sqlx::query!(
            "INSERT INTO kavasam_reports (hash_id, reported_by) 
             VALUES (
               (SELECT ID from kavasam_hashes WHERE id_type = $1 AND hash = $2),
               (SELECT ID from kavasam_users WHERE public_key = $3)
              )
             ON CONFLICT(hash_id, reported_by) DO NOTHING;
             ",
            &id_type,
            &hash,
            &pkey
        )
        .execute(&data.db)
        .await
        .unwrap();

        for tag in &tags {
            // TODO possible unique constraint violation"
            sqlx::query!(
                "INSERT INTO kavasam_report_tags (hash_id, tag_id) 
             VALUES (
               (SELECT ID from kavasam_hashes WHERE id_type = $1 AND hash = $2),
               (SELECT ID from kavasam_tags WHERE name = $3)
              )
             ON CONFLICT(hash_id, tag_id) DO NOTHING;
             ",
                &id_type,
                &hash,
                &tag,
            )
            .execute(&data.db)
            .await
            .unwrap();
        }
    }

    HttpResponse::Ok()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct QueryAllReportedByRequest {
    pub id_type: Option<IDType>,
    pub public_key: PublicKey,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StrippedReport {
    pub id_type: IDType,
    pub hash: String,
}

#[my_codegen::post(path = "ROUTES.get_all_reported_by")]
pub async fn get_all_reported_by(
    payload: web::Json<QueryAllReportedByRequest>,
    data: AppData,
) -> impl Responder {
    let pkey = payload.public_key.asci_armor();

    if let Some(id_type) = &payload.id_type {
        struct StrippedReportInner {
            hash: String,
        }

        let mut hashes = sqlx::query_as!(
            StrippedReportInner,
            "SELECT kavasam_hashes.hash
                FROM kavasam_hashes
                INNER JOIN
                    kavasam_reports
                ON
                    kavasam_hashes.ID = kavasam_reports.hash_id
                INNER JOIN 
                    kavasam_users
                ON
                    kavasam_users.ID = kavasam_reports.reported_by
                WHERE
                    kavasam_users.public_key = $1
                AND
                    kavasam_hashes.id_type = $2
                ",
            &pkey,
            &serde_json::to_string(id_type).unwrap(),
        )
        .fetch_all(&data.db)
        .await
        .unwrap();
        let resp: Vec<StrippedReport> = hashes
            .drain(..)
            .map(|h| StrippedReport {
                hash: h.hash,
                id_type: id_type.clone(),
            })
            .collect();
        HttpResponse::Ok().json(resp)
    } else {
        struct StrippedReportInner {
            hash: String,
            id_type: String,
        }

        let mut hashes = sqlx::query_as!(
            StrippedReportInner,
            "SELECT kavasam_hashes.hash, kavasam_hashes.id_type
                FROM kavasam_hashes
                INNER JOIN
                    kavasam_reports
                ON
                    kavasam_hashes.ID = kavasam_reports.hash_id
                INNER JOIN 
                    kavasam_users
                ON
                    kavasam_users.ID = kavasam_reports.reported_by
                WHERE
                    kavasam_users.public_key = $1
                ",
            &pkey,
        )
        .fetch_all(&data.db)
        .await
        .unwrap();
        let resp: Vec<StrippedReport> = hashes
            .drain(..)
            .map(|h| StrippedReport {
                hash: h.hash,
                id_type: serde_json::from_str(&h.id_type).unwrap(),
            })
            .collect();
        HttpResponse::Ok().json(resp)
    }
}

pub fn services(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(report);
    cfg.service(get_all_reported_by);
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::*;
    use libkavasam::id::Identity;
    use libkavasam::{IDType, ReportMessageBuilder};

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

    pub mod utils {
        use std::sync::Arc;

        use super::*;
        use crate::Data;

        type UtilData = Arc<Data>;

        pub const ID: &[u8] = b"foo@example.com";
        pub const TAGS: [&str; 3] = ["bank fraud", "insurance spam", "stalker"];

        pub fn get_msg() -> (Identity, ReportMessage) {
            let id = Identity::new();
            let tags = TAGS.iter().map(|s| s.to_string()).collect();
            let msg = ReportMessageBuilder::default()
                .id_type(IDType::Email)
                .hashes(&id, ID)
                .tags(tags)
                .build()
                .unwrap();
            (id, msg)
        }

        pub async fn add_report(msg: &ReportMessage, data: &UtilData) {
            let app = get_app!(data).await;
            let add_token_resp = actix_web::test::call_service(
                &app,
                post_request!(&msg, ROUTES.report).to_request(),
            )
            .await;
            assert_eq!(add_token_resp.status(), StatusCode::OK);
        }

        pub async fn query_report(id: &Identity, msg: &ReportMessage, data: &UtilData) {
            fn verify(msg: &ReportMessage, resp: &[StrippedReport]) {
                assert!(resp.iter().all(|x| x.id_type == msg.id_type));
                msg.hashes.iter().for_each(|h| {
                    let hash = h.ascii_armor();
                    let found = resp.iter().find(|x| x.hash == hash.hash);
                    assert!(found.is_some());
                });
            }

            let query = QueryAllReportedByRequest {
                id_type: None,
                public_key: id.pub_key(),
            };

            let mut query_2 = query.clone();
            query_2.id_type = Some(msg.id_type.clone());

            let app = get_app!(data).await;

            for q in [query, query_2] {
                let query_report_resp = actix_web::test::call_service(
                    &app,
                    post_request!(&q, ROUTES.get_all_reported_by).to_request(),
                )
                .await;
                assert_eq!(query_report_resp.status(), StatusCode::OK);
                let res: Vec<StrippedReport> =
                    actix_web::test::read_body_json(query_report_resp).await;
                verify(msg, &res);
            }
        }

        pub async fn delelte_user(id: &Identity, data: &Data) {
            let pkey = id.pub_key().asci_armor();
            let _ =
                sqlx::query!("DELETE FROM kavasam_users WHERE public_key = ($1)", &pkey)
                    .execute(&data.db)
                    .await;
        }

        pub async fn delete_hashes(msg: &ReportMessage, data: &Data) {
            for hash in msg.hashes.iter() {
                let hash = hash.ascii_armor().hash;
                let _ =
                    sqlx::query!("DELETE FROM kavasam_hashes WHERE hash = ($1)", &hash)
                        .execute(&data.db)
                        .await;
            }
        }
    }

    #[actix_rt::test]
    async fn test_report() {
        let (id, msg) = utils::get_msg();
        let data = Data::new().await;
        utils::delelte_user(&id, &data).await;
        utils::delete_hashes(&msg, &data).await;

        utils::add_report(&msg, &data).await;
    }

    #[actix_rt::test]
    async fn test_query() {
        let (id, msg) = utils::get_msg();
        let data = Data::new().await;
        utils::delete_hashes(&msg, &data).await;
        utils::delelte_user(&id, &data).await;

        utils::add_report(&msg, &data).await;
        utils::query_report(&id, &msg, &data).await;
    }
}
