# p(air) prog(rammer)
pprog is an LLM based pair programmer for working on coding projects.  it can generate, edit and answer questions about your code.

This is experimental and unstable code, it may change at any time.  It has solid support for Claude models through the Anthropic API, as well as OpenAI.  Still working on support for o1 as the lack of system prompt makes it more difficult to implement.  The program should work with any OpenAI compatible API by assigning corresponding api url in config.  Some examples of different configs can be found in `examples` directory.  The tooling logic is intended to be as simple as possible so the model has more flexibility to maneuver.  I am always open to suggestions!

## prereqs
- rust
- browser
- git

To install Rust, go to their [website](https://www.rust-lang.org/).

## install
```
cargo install pprog
```

## usage
To use `pprog`, `cd` into the directory of an existing or template project.  `pprog` depends on `git` and also uses `.gitignore` to communicate the available files to LLM, so the project must have `git` initialized. For this example, we'll create a basic NodeJS project.
```
mkdir example-project
cd example-project
npm init -y && git init
pprog init
```
This will generate a config file `pprog.toml` with sensible defaults depending on the type of project.  For this example the `pprog.toml` will contain
```
provider = "anthropic"
model = "claude-3-5-haiku-latest"
check_cmd = "timeout 3s node index.js"
api_url = "https://api.anthropic.com/v1/messages"
api_key = "..." // if ANTHROPIC_API_KEY env var is set then it will automatically add it
max_context = 128000
max_output_tokens = 8096
```
The program that generates and edits code in the backend uses the `check_cmd` to check compilation or successful operation.  In this case `timeout 3s node index.js` will be run to check for any errors and if they exist new changes will be made to correct them until all errors are gone.  You're free to change `check_cmd` to anything you want for the given program.  For compiled projects using a langauge like Rust, `check_cmd` would be `"cargo check"`.  For intepreted languages it will depend on the type of program.  For long lived programs like a web server, you can use the timeout trick above (`gtimeout` on Macbooks) to check for any initial runtime errors.  For intepreted programs that are not long lived simply running the program (like `node short-lived-script.js`) should work.  Note that if not using a timeout for interpreted programs, the chat will not continue until the program completes.

An Anthropic account is assumed on init, but OpenAI-compatible APIs can be used as well.  For example, to use OpenAI you can change config to 
```
provider = "openai"
model = "gpt-4o"
check_cmd = "timeout 3s node index.js"
api_url = "https://api.openai.com/v1/chat/completions"
api_key = "<OEPNAI API KEY>"
max_context = 100000
max_output_tokens = 8096
```
To run enter
```
pprog serve
```
and then enter `http://localhost:8080` in your browser.  A chat interface will load and you can begin making changes to your code.  For example, in this example project you can type in a message like `Create an index.js file with basic express server` and it will create file and check that it runs properly by using `check_cmd` command.  Then another message like `Add GET /ping endpoint` and it will make changes to the code and check again.  You may also questions about the code.  

You can run `pprog serve` for multiple projects at the same time by assigning different ports
```
pprog serve --port 3002
```

# tools
`pprog` uses a very small set of tools to make changes.  currently it has four.
```
read_file - read entire file contents
write_file - replace entire file with contents
execute - run general bash, sometimes used by agent to install packages when check fails
compile_check - check for compilation errors, or for interpreted programs checks runtime errors on startup
```

# tips and warnings
- The system prompt includes instructions to not change any files outside of the root of the project but this is not strictly guaranteed.  It has not gone outside the root of a project once, but if you prompt it to it possibly could.
- If using Anthropic/OpenAI models it can get expensive, but is usually very effective.  When using Sonnet 3.5 a single code change request routinely cost 0.20 USD or more.  This is because the program is constantly reading/writing entire files to satisfy each request.  I shudder to use Opus and haven't even tried.  Haiku 3.5 seems to be a good trade-off, usually costing a few cents per change of a medium sized project.  I normally use Haiku.  DeepSeek V3 is dirt cheap and can be effective but less so, usually requires multiple attempts where Sonnet will one-shot it.  OpenAI models can be effective, usually gpt-4o-mini as gpt-4o gets throttled on rate limits almost immediately unless you can raise them.
- It doesn't use RAG and I'm thinking of implementing it or some other chunking logic but in general each file in the project should be considered as a chunk.  This means you want to refactor frequently and liberally.  Since the program can only read and write entire files you don't want them to get too big.  My take on this is that attention mechanisms aren't effective at long range and trying to game the context limit usually results in poor performance.
- Make sure to commit and push changes frequently.  It's ok to sometimes make multiple changes before committing but if it's going to be a large change then best to commit before making them.  The program does not make commits on each change as I think that should be left to the user and many times the changes will not be what you want, so you need to run `git restore .` or the like.  I usually run git commands through the chat by prompting such as `Great, commit all these changes` or `No this is wrong, please restore all changes.` so that the model can attend to it. 
- The system prompt notes that the user may ask questions and the model is usually good at figuring out when a question without needed code changes is asked, but I usually prepend question messages with 'Question: ' to steer the model.  In general I've found that when I ask questions about the codebase it reguarly decides to make changes.  Still trying to figure out how to steer this behavior better.
- It's in the system prompt, but models will usually do many file writes and get confused about when a compile check should be run.  Explicitly ask for a compile check and it will run and attempt to fix errors.
- You'll still have to do some coding, sorry anon.

happy hacking!
