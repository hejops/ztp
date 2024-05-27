use actix_web::HttpResponse;
use actix_web_flash_messages::FlashMessage;

use crate::session_state::TypedSession;
use crate::utils::error_500;
use crate::utils::redirect;

pub async fn logout(session: TypedSession) -> Result<HttpResponse, actix_web::Error> {
    if session.get_user_id().map_err(error_500)?.is_some() {
        session.logout();
        FlashMessage::info("You have successfully logged out.").send();
    }
    Ok(redirect("/login"))
}
