# IOTA Pow Box

This project provides IOTA PoW-as-a-Service 

Standard IRI `attachToTangle` requests returns a JSON struct containing a `jobId`. Querying the server at `localhost:8000/<jobId>`
returns either `"Not ready yet"` or a standard `attachToTangle` response with a PoW-ed trytes attribute.

To build this project, you'll need openssl-devel install (on linux) or equivalent openssl development libraries.
