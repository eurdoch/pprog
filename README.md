# p(air) prog(rammer)
pprog is an LLM based pair programmer for generating and editing code in individual projects

## prereqs
- Rust (cargo)
- Browser
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
model = "claude-3-5-sonnet-latest"
check_cmd = "node index.js"
base_url = "https://api.anthropic.com/v1"
api_key = "..." // if ANTHROPIC_API_KEY env var is set then it will automatically add it
```
The program that generates and edits code in the backend uses the `check_cmd` to check compilation or successful operation.  In this case `node index.js` will be run to check for any errors in code changes and then loop to fix these changes if they exist.  For compiled projects using a langauge like Rust, `check_cmd` would be `"cargo check"`.  An Anthropic account is assumed on init, but OpenAI-compatible APIs can be used as well.  For example, to use DeepSeek you can change config to 
```
model = "deepseek-chat"
check_cmd = "node index.js"
base_url = "https://api.deepseek.com/v1"
api_key = "<DEEPSEEK API KEY>"
```
With config set, to start the server run 
```
pprog serve
```
and then enter `http://localhost:8080` in your browser.  A chat interface will load and you can begin making changes to your code.  For example, in this example project you can type in a message like `Create an index.js file with basic express server` and it will create file and check that it runs properly by using `check_cmd` command.  Then another message like `Add GET /ping endpoint` and it will make changes to the code and check again.  You may also questions about the code or anything in general.

You can run `pprog serve` for multiple projects at the same time by assigning different ports
```
pprog serve --port 3002
```

# tools
`pprog` uses a very small set of tools to make changes.  currently it has four existing tools
```
read_file - read entire file contents
write_file - replace entire file with contents
execute - run general bash, sometimes used by agent to install packages when check fails
compile_check - check for compilation errors, or for interpreted programs checks runtime errors on startup
```

# tips and warnings
- This is experimental and unstable code, it may change at any time.
- The system prompt includes instructions to not change any files outside of the root of the project but this is not stricly guaranteed.  Proceed with caution!
- If using Anthropic/OpenAI models it can get expensive, but is usually very effective.  When using Sonnet 3.5 a single code change request routinely cost 0.20 USD or more.  This is because the program is constantly reading/writing entire files to satisfy each request.  Haiku 3.5 seems to be a good trade-off, usually costing a few cents per change of a medium sized project.  DeepSeek is dirt cheap but doesn't seem as effective.  OpenAI models can be effective, but usually get throttled by rate limits almost immediately.
- Make sure to commit and push changes frequently.  It's ok to sometimes make multiple changes before committing but if it's going to be a large change then best to commit before making them.
- You'll still have to do some coding

happy hacking!
