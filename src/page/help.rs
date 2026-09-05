use sycamore::prelude::*;

pub fn view() -> View {
    view! {
        section(class="hero is-medium ml-6") {
            div(class="hero-body") {
                h1(class="title is-size-1") {
                    "Khana Time Tracker"
                    span(class="tag is-info is-medium ml-3") { (format!("v{}", khanatime::APP_VERSION)) }
                }
                span {
                    r"
                    For the moment this works best on a PC, as it is designed
                    for keyboard operation.

                    This is a standalone application, the data is stored on your
                    device, it is NOT sent back to a central server.  It will work
                    offline, you can bookmark the URL.  For best results, Install
                    it as an app in your browser.  On a PC there is an icon in the
                    URL bar. On a phone there in an item to install as an app in the
                    menu.

                    If you are sent the link via facebook, it is best to open it
                    in a full browser.  There will be a open in Chrome or similar
                    item in the menu.
                    "
                }
                div(class="box mt-5") {
                    h2(class="title is-5") { "Stage status colours (FIA rally results)" }
                    p(class="help") {
                        "The per-test tag on Home shows a test's state:"
                    }
                    div(class="mt-2") {
                        div(class="tags has-addons") { span(class="tag is-info") { "Blue" } span(class="tag") { "Completed" } }
                        div(class="tags has-addons") { span(class="tag is-warning") { "Orange" } span(class="tag") { "Running" } }
                        div(class="tags has-addons") { span(class="tag is-light") { "Grey" } span(class="tag") { "To run" } }
                        div(class="tags has-addons") { span(class="tag is-info kt-struck") { "Blue" } span(class="tag") { "Cancelled (struck out)" } }
                    }
                }
            }
        }
    }
}
