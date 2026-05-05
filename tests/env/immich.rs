use retrodate::{Asset, ExifInfo, Patch, Query, Results, Search};
use std::thread::JoinHandle;
use tiny_http::{HeaderField, ListenAddr, Method, Request, Response, ResponseBox, Server};

type Handler = Box<dyn Fn(&mut Request, &mut [Asset]) -> Option<ResponseBox> + Send>;

pub struct Immich {
    server: Server,
    state: Vec<Asset>,

    handlers: Vec<Handler>,
    default_handlers: Vec<Handler>,
}

impl Immich {
    pub fn with_assets(assets: Vec<Asset>) -> Self {
        let server = Server::http("127.0.0.1:0").unwrap();
        Self {
            server,
            state: assets,
            handlers: Vec::new(),
            default_handlers: vec![
                Box::new(post_search_metadata),
                Box::new(get_asset),
                Box::new(put_asset),
            ],
        }
    }

    pub fn listening_address(&self) -> ListenAddr {
        self.server.server_addr()
    }

    #[allow(dead_code)]
    pub fn add_handler(&mut self, handler: Handler) {
        self.handlers.push(handler)
    }

    pub fn handle_single_request(&mut self) {
        let mut request = self.server.recv().unwrap();

        println!("Received {} {}", request.method(), request.url());

        if request
            .headers()
            .iter()
            .find(|header| header.field == HeaderField::from_bytes(r"x-api-key").unwrap())
            .is_none()
        {
            let response = Response::empty(401);
            request.respond(response).unwrap();
            return;
        }
        for handler in &self.handlers {
            if let Some(response) = handler(&mut request, &mut self.state) {
                request.respond(response).unwrap();
                return;
            }
        }

        for handler in &self.default_handlers {
            if let Some(response) = handler(&mut request, &mut self.state) {
                request.respond(response).unwrap();
                return;
            }
        }

        let response = Response::empty(404);
        request.respond(response).unwrap();
    }

    pub fn spawn(mut self) -> JoinHandle<()> {
        std::thread::spawn(move || {
            loop {
                self.handle_single_request();
            }
        })
    }
}
/// Handle a HTTP POST on /api/search/metadata
fn post_search_metadata(request: &mut Request, state: &mut [Asset]) -> Option<ResponseBox> {
    if request.method() != &Method::Post {
        return None;
    }

    if request.url() != "/api/search/metadata" {
        return None;
    }

    let Ok(query @ Query { .. }) = serde_json::from_reader(request.as_reader()) else {
        return Some(Response::empty(400).boxed());
    };

    let matches: Vec<Asset> = state
        .iter()
        .filter_map(|asset| {
            if asset.original_file_name.contains(&query.original_file_name) {
                Some(asset.clone())
            } else {
                None
            }
        })
        .collect();

    // The total number of matches.
    let match_count = matches.len();
    let page_size = 2;
    let page: usize = query.page.unwrap_or(String::from("1")).parse().unwrap();
    let next_page = {
        if page * page_size < match_count {
            Some(format!("{}", page + 1))
        } else {
            None
        }
    };

    // The filtered matches for this page.
    let matches = matches
        .into_iter()
        .enumerate()
        .filter_map(|(index, asset)| {
            let offset = (page - 1) * page_size;

            // The pages are starting from 1, whereas
            // the assets use  zero based indexing.

            if (index) < offset {
                return None;
            }

            if (index) >= (offset + page_size) {
                return None;
            }
            Some(asset)
        })
        .collect();

    let Ok(search_results) = serde_json::to_string_pretty(&Search {
        assets: Results {
            total: match_count,
            items: matches,
            next_page,
        },
    }) else {
        return Some(Response::empty(500).boxed());
    };

    Some(Response::from_string(search_results).boxed())
}

fn get_asset(request: &mut Request, state: &mut [Asset]) -> Option<ResponseBox> {
    if request.method() != &Method::Get {
        return None;
    }
    if !request.url().starts_with("/api/assets/") {
        return None;
    }

    let id = request
        .url()
        .split('/')
        .next_back()
        .expect("This shouldn't fail since earlier we check if URL starts with a /");

    if let Some(asset) = state.iter().find(|asset| asset.id == id) {
        let search_results = serde_json::to_string_pretty(&asset).unwrap();
        return Some(Response::from_string(search_results).boxed());
    };

    Some(Response::empty(404).boxed())
}

fn put_asset(request: &mut Request, state: &mut [Asset]) -> Option<ResponseBox> {
    if request.method() != &Method::Put {
        return None;
    }
    if !request.url().starts_with("/api/assets/") {
        return None;
    }

    let Ok(patch @ Patch { .. }) = serde_json::from_reader(request.as_reader()) else {
        return Some(Response::empty(400).boxed());
    };
    let id = request
        .url()
        .split('/')
        .next_back()
        .expect("This shouldn't fail since earlier we check if URL starts with a /");

    let Some(ref mut asset) = state.iter_mut().find(|asset| asset.id == id) else {
        return Some(Response::empty(404).boxed());
    };

    if let Some(exif_info) = &mut asset.exif_info {
        exif_info.date_time_original = Some(patch.date_time_original);
    } else {
        asset.exif_info = Some(ExifInfo {
            date_time_original: Some(patch.date_time_original),
        });
    }

    let Ok(asset) = serde_json::to_string_pretty(&asset) else {
        return Some(Response::empty(500).boxed());
    };
    Some(Response::from_string(asset).boxed())
}
