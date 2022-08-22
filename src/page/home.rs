// use crate::Urls;
use seed::{prelude::*, *};

pub fn view<Ms>() -> Node<Ms> {
    section![
        C!["hero", "is-medium", "ml-6"],
        div![
            C!["hero-body"],
            h1![C!["title", "is-size-1"], "Khana Time Tracker",],
            span![
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
            ]
        ]
    ]
}
