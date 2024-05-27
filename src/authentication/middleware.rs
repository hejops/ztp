use std::ops::Deref;

use actix_web::body::MessageBody;
use actix_web::dev::ServiceRequest;
use actix_web::dev::ServiceResponse;
use actix_web::error::InternalError;
use actix_web::FromRequest;
use actix_web::HttpMessage;
use actix_web_lab::middleware::Next;
use uuid::Uuid;

use crate::session_state::TypedSession;
use crate::utils::error_500;
use crate::utils::redirect;

// `Clone` grants `.into_inner`, the other traits are not strictly necessary
#[derive(Clone)] //, Copy, Debug)]
pub struct UserId(Uuid);

// impl Display for UserId {
//     fn fmt(
//         &self,
//         f: &mut std::fmt::Formatter<'_>,
//     ) -> std::fmt::Result {
//         self.0.fmt(f)
//     }
// }

// basically just for unpacking the inner Uuid type
impl Deref for UserId {
    type Target = Uuid;
    fn deref(&self) -> &Self::Target { &self.0 }
}

// essentially just a type coercion for get_user_id
// fn get_user_id(&self) -> Result<Option<Uuid>, SessionGetError>
//
// i don't necessarily agree with this function's unclear intent (why does it
// also return the id?) but let's roll with it for now...
// pub fn reject_anonymous_users(session: TypedSession) -> Result<Uuid,
// actix_web::Error> {     match session.get_user_id().map_err(error_500)? {
//         Some(user_id) => Ok(user_id),
//         None => {
//             // Ok(redirect("/login"));
//             let resp = redirect("/login");
//             let err = anyhow::anyhow!("You must be logged in to access this
// resource.");             Err(InternalError::from_response(err, resp).into())
//         }
//     }
// }

/// Since authentication will be used very often, it makes sense to turn this
/// protocol into a middleware that persists across the entire app. However,
/// since middlewares generally only "take" data (without expecting to return
/// it), data can be embedded in request.
///
/// For more details, refer to the documentation for
/// `actix_web_lab::middleware::from_fn`
pub async fn reject_anonymous_users(
    // notice that a `ServiceRequest`'s fields can be passed directly into `from_request`, which we
    // can then use to construct our `TypedSession`
    mut req: ServiceRequest,
    next: Next<impl MessageBody>,
) -> Result<ServiceResponse<impl MessageBody>, actix_web::Error> {
    let (raw_req, payload) = req.parts_mut();
    let session = TypedSession::from_request(raw_req, payload).await?;

    match session.get_user_id().map_err(error_500)? {
        Some(user_id) => {
            // Ok(user_id)
            req.extensions_mut().insert(UserId(user_id));
            next.call(req).await
        }
        None => {
            // Ok(redirect("/login"));
            let resp = redirect("/login");
            let err = anyhow::anyhow!("You must be logged in to access this resource.");
            Err(InternalError::from_response(err, resp).into())
        }
    }
    // todo!()
}
