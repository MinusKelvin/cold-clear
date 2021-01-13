# FAQ:

### Can I use Cold Clear on tetr.io?

No. Only MinusKelvin can run the Bot on tetr.io. The tetr.io version is not
public to prevent abuse.

### How can I run Cold Clear?

- If you just want to fight it **online** with no setup needed it there is
  [cold-clear-web](https://minuskelvin.net/cold-clear/).  **Note:** changing
the default configuration is not supported (*yet*).

- To run it **offline** you can download the [standalone
  version](https://github.com/MinusKelvin/cold-clear/releases/download/v0.1-alpha5/cold-clear.exe).
After Running if for the first time it creates the file `options.yaml` in the
same directory that you put the `cold-clear.exe` in. You should edit that file
to change the default configuration.

- To fight it in **Puyo Puyo Tetris (PPT)** you should check the
  [#ppt-releases](https://discord.com/channels/708203305494642718/708203963421294673)
channel on the Cold Clear Discord Server.  **Note:** Make sure to also install
the ScpDriver that is linked in the channel and to have Witch unlocked!

- See [How do I use MinusBot](#how-do-i-use-minusbot) and [Compiling from
  source](#compiling-from-source) for more options.

### How do I use MinusBot?

There is also a Discord bot hosted on the Cold Clear Discord Server that can
run Cold Clear on [fumen](https://harddrop.com/fumen/) quizzes. To use it just
type `-cc <link to a fumen quiz>` in the chat.  **Note:** You can run it
multiple times to get different results (sometimes).

### Compiling from source

To compile it from source make sure you have
[Rust](https://www.rust-lang.org/tools/install) properly installed (don't
forget to install the [Visual C++ Build
Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/) if you're on
Windows).  Then run `cargo run --release -p gui` in the cold-clear directory to
compile and run the standalone client.
