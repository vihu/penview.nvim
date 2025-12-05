UNAME := $(shell uname)
ifeq ($(UNAME), Darwin)
    EXT := dylib
else
    EXT := so
endif

build:
	cargo build --release
	mkdir -p rust/websocket-ffi/lua
	cp target/release/libwebsocket_ffi.$(EXT) rust/websocket-ffi/lua/websocket_ffi.so

build-debug:
	cargo build
	mkdir -p rust/websocket-ffi/lua
	cp target/debug/libwebsocket_ffi.$(EXT) rust/websocket-ffi/lua/websocket_ffi.so

clean:
	cargo clean
	rm -rf rust/websocket-ffi/lua
