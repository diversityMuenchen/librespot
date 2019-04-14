use std::thread;
use futures;
use futures::{future, Future};
use core::session::Session;
//use rocket;
//use rocket_contrib::json::JsonValue;
//use rocket::{Rocket, State};
//use rocket::http::RawStr;
//use rocket::response::status;

use hyper::{Body, Response, StatusCode};
use gotham::handler::{HandlerError, HandlerFuture, IntoHandlerError, IntoResponse};
use gotham::helpers::http::response::create_response;
use gotham::middleware::state::StateMiddleware;
use gotham::pipeline::single::single_pipeline;
use gotham::pipeline::single_middleware;
use gotham::router::{builder::*, Router};
use gotham::state::{FromState, State};

use metadata::{FileFormat, Metadata, Track, Album, Artist};
use core::spotify_id::SpotifyId;

use std::sync::{Arc, RwLock};
use serde::ser::{Serialize, Serializer, SerializeStruct, SerializeSeq};

type SleepFuture = Box<Future<Item = metadata::Track, Error = HandlerError> + Send>;

pub struct Service {
    thread_handle: Option<thread::JoinHandle<()>>,
}

#[derive(Clone, StateData)]
pub struct ServiceState {
    session: Arc<RwLock<Session>>,
}

#[derive(Deserialize, StateData, StaticResponseExtender)]
struct PathExtractor {
    id: String,
}

impl Service {
    pub fn new(session: Session,) -> Service {
        let handle = thread::spawn(move || {
            debug!("new Service[{}]", session.session_id());

            let service_state = ServiceState {
                session: Arc::new(RwLock::new(session)),
            };

            debug!("new ServiceTask[{}]", service_state.session.read().unwrap().session_id());
            let addr = "127.0.0.1:8000";
            println!("Listening for requests at http://{}", addr);
            gotham::start(addr, router(service_state));
//            let rocket_app = rocket::ignite().mount("/api", routes![track, artist, queue]).manage(service_state);
//            rocket_app.launch();
        });

        Service {
            thread_handle: Some(handle),
        }
    }
}

pub struct SpotifyIdSer(SpotifyId);

impl Serialize for SpotifyIdSer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        let SpotifyIdSer(x) = self;
        serializer.serialize_str(&x.to_base62())
    }
}

pub struct SpotifyIdVecSer(Vec<SpotifyId>);

impl Serialize for SpotifyIdVecSer {
//    state.serialize_field("artist", &x.artists.iter().map(|x| SpotifyIdSer(*x)).collect::<Vec<SpotifyIdSer>>())?;
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        let SpotifyIdVecSer(x) = self;
        let mut seq = serializer.serialize_seq(Some(x.len()))?;
        for e in x {
            seq.serialize_element(&SpotifyIdSer(*e))?;
        }
        seq.end()
    }
}

trait New<X, Y> {
    fn new(t: Y) -> X;
}

pub struct TrackSer(Track);

impl New<TrackSer, Track> for TrackSer {
    fn new(track: Track) -> TrackSer {
        TrackSer(track)
    }
}

impl Serialize for TrackSer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        let TrackSer(x) = self;
        let mut state = serializer.serialize_struct("track", 7)?;
        state.serialize_field("id", &SpotifyIdSer(x.id))?;
        state.serialize_field("name", &x.name)?;
        state.serialize_field("artist", &x.artists.iter().map(|x| SpotifyIdSer(*x)).collect::<Vec<SpotifyIdSer>>())?;
        state.serialize_field("album", &SpotifyIdSer(x.album))?;
        state.serialize_field("duration", &x.duration)?;
        state.serialize_field("alternatives", &x.alternatives.iter().map(|x| SpotifyIdSer(*x)).collect::<Vec<SpotifyIdSer>>())?;
        state.serialize_field("available", &x.available)?;
        state.end()
    }
}


pub struct AlbumSer(Album);

impl New<AlbumSer, Album> for AlbumSer {
    fn new(album: Album) -> AlbumSer {
        AlbumSer(album)
    }
}

impl Serialize for AlbumSer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        let AlbumSer(x) = self;
        let mut state = serializer.serialize_struct("track", 4)?;
        state.serialize_field("id", &SpotifyIdSer(x.id))?;
        state.serialize_field("name", &x.name)?;
        state.serialize_field("artist", &x.artists.iter().map(|x| SpotifyIdSer(*x)).collect::<Vec<SpotifyIdSer>>())?;
        state.serialize_field("tracks", &x.tracks.iter().map(|x| SpotifyIdSer(*x)).collect::<Vec<SpotifyIdSer>>())?;
//        state.serialize_field("covers", &x.artists.iter().map(|x| SpotifyIdSer(*x)).collect::<Vec<SpotifyIdSer>>())?;
        state.end()
    }
}

pub struct ArtistSer(Artist);

impl New<ArtistSer, Artist> for ArtistSer {
    fn new(artist: Artist) -> ArtistSer {
        ArtistSer(artist)
    }
}


impl Serialize for ArtistSer {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: Serializer,
    {
        let ArtistSer(x) = self;
        let mut state = serializer.serialize_struct("track", 3)?;
        state.serialize_field("id", &SpotifyIdSer(x.id))?;
        state.serialize_field("name", &x.name)?;
        state.serialize_field("top_tracks", &x.top_tracks.iter().map(|x| SpotifyIdSer(*x)).collect::<Vec<SpotifyIdSer>>())?;
        state.end()
    }
}

fn get_handler<X: Serialize + New<X,Y>, Y: Metadata>(mut state: State) -> Box<HandlerFuture> {
    let path = PathExtractor::take_from(&mut state);
    match SpotifyId::from_base62(&path.id) {
        Ok(spotifyid) => {
            let service_state = ServiceState::take_from(&mut state);
            let session = service_state.session.read().unwrap();
            let metadata_request = Y::get(&session, spotifyid);
            Box::new(metadata_request.then(move |result| match result {
                Ok(meta_response) => {
//                        println!("track name {}", meta_response.name);
                    let res = create_response(&state, StatusCode::OK, mime::TEXT_PLAIN, serde_json::to_string(&(X::new(meta_response))).unwrap());
                    Ok((state, res))
                },
                Err(E) => {
                    let res = create_response(&state, StatusCode::NOT_FOUND, mime::TEXT_PLAIN, "not found");
                    println!("not found");
                    Ok((state, res))
                },
            }))
        },
        Err(e) => {
            let resp = create_response(&state, StatusCode::BAD_REQUEST, mime::TEXT_PLAIN, "wrong input");
            Box::new(future::ok((state, resp)))
        }
    }
}

fn router(service_state: ServiceState) -> Router {
    // create the counter to share across handlers
//    let counter = RequestCounter::new();

    // create our state middleware to share the counter
    let middleware = StateMiddleware::new(service_state);

    // create a middleware pipeline from our middleware
    let pipeline = single_middleware(middleware);

    // construct a basic chain from our pipeline
    let (chain, pipelines) = single_pipeline(pipeline);

    // build a router with the chain & pipeline
    build_router(chain, pipelines, |route| {
        route.get("/track/:id").with_path_extractor::<PathExtractor>().to(get_handler::<TrackSer, Track>);
        route.get("/album/:id").with_path_extractor::<PathExtractor>().to(get_handler::<AlbumSer, Album>);
        route.get("/artist/:id").with_path_extractor::<PathExtractor>().to(get_handler::<ArtistSer, Artist>);
    })
//    build_simple_router(|route| {
//        route
//            // Note the use of :name variable in the path defined here. The router will map the
//            // second (and last) segment of this path to the field `name` when extracting data.
//            .get("/track/:id")
//            // This tells the Router that for requests which match this route that path extraction
//            // should be invoked storing the result in a `PathExtractor` instance.
//            .with_path_extractor::<PathExtractor>()
//            .to(get_product_handler);
//    })
}

//#[get("/track/<id>")]
//pub fn track(id: String, state: State<ServiceState>) -> JsonValue {
//    match SpotifyId::from_base62(&id) {
//        Err(E) => json!({"error": "bad id",}),
//        Ok(track_id) => {
//            match Track::get(&state.session, track_id).wait() {
//                Err(E) => json!({"error": "not found",}),
//                Ok(track) => {
//                    json!({
//                              "track": id,
//                              "name": track.name,
//                              "session_id": state.session.session_id(),
//                        })
//                }
//            }
//        }
//    }
////    json!({})
//}
//
//#[get("/artist/<id>")]
//pub fn artist(id: String) -> JsonValue {
//    json!({
//        "artist": id,
//    })
//}
//
//#[get("/queue")]
//pub fn queue() -> JsonValue {
//
//    json!({})
//}