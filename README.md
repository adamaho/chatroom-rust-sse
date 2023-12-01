# Rust Sse Message Bus

Example implementation of a message bus in rust for sending sse events down to connected clients.

## Pitfalls

It leaks memory due to the use of the `'static` lifetime on the `send` function. lol. I am still a rust nooby so I dont know how to fix it.

But meh, it works :)


