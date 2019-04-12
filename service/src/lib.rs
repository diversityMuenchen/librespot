#[macro_use] extern crate log;

extern crate gotham;
#[macro_use]
extern crate gotham_derive;
extern crate hyper;
extern crate mime;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;

//#[macro_use] extern crate rocket;
//#[macro_use] extern crate rocket_contrib;

extern crate futures;

extern crate librespot_core as core;
extern crate librespot_metadata as metadata;

pub mod service;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
