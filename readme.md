# rat invaders
its a space invaders type game made in ratatui to be ran over ssh. the novelty is not in the game but in the adapter. with this you can run ratatui and build any tui app with rust and run them over ssh.

# prerequisites
- rust
- rustup
- cargo

# features
- state and init management
- rendering loop
- clear, fg, bg, cursor pos ansii support

# demo

[Demo video](https://github.com/user-attachments/assets/f64a5b48-33b3-4c69-94ad-675e39888ffe)

you can do `ssh beanoni.xyz -p 2223`

## technical rant

It's kinda interesting. I might switch over to Rust at this rate. You can Frankenstein together pretty much any libraries pretty easily.

I'm using a channel to send data from Ratatui whenever it tries to draw, clear, or set the cursor. This data is sent over to an SSH output framebuffer through a channel.

The architecture looks roughly like this:

```text
Client<S>
│
├── state: S
│   └── Application state / state data
│
├── renderer: Arc<RenderFunction<S>>
│   └── Tick / render function
│
├── input_handler: Arc<InputHandler<S>>
│   └── Input handling function
│
├── init_state_callback: Option<Box<InitStateCallback<S>>>
│   └── Ready / initialization callback
│
└── ratatui_terminal: Option<Arc<Mutex<Terminal<RatatuiAdapter>>>>
    │
    └── ratatui::Terminal
        │
        └── RatatuiAdapter
            │
            ├── draw()
            ├── clear()
            ├── cursor operations
            │
            └── edits framebuffer
                │
                ▼
             ANSI output
                │
                ▼
             Channel
                │
                ▼
             SSH client

this is wrapped over by a ClientHandler<S> to give access to shared ownership of Client<S> (required for callbacks, cant just send this to the main loop)

finally we then implement the ssh handler in the ClientHandler<S> so it covers everything and we can connect ssh to client 


```


# build
```sh
cargo build
```

# AI declaration
ai was used to help me find the function and norms for this project. i didnt write code using it 
