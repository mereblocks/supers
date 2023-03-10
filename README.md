# `supers`

`supers` is a programmable supervisor for long-running programs. 

In `supers`, an application is a set of programs with restart policies. `supers` reads the application's configuration of programs and starts them up. Then, it presents a basic administrative API over HTTP that can be used to control the status of the running programs.

## Build and Run

The configuration for `supers` lives in a TOML file. Its default location is `/etc/supers/conf.toml`. The environment variable `SUPERS_CONF_FILE` overrides the default location.

When `supers` doesn't find a configuration file in any of the locations, it starts with an empty list of programs, listening in `localhost:8080`.

Build and run with:

```bash
cargo build
./supers
```

## Developing supers

### Using Nix (recommended)

1. Install [Nix](https://nixos.org/download.html).
2. Run `nix build` for building the crate.
3. Run `nix develop` to spawn a shell with the tools needed for developing `supers`.

After Step 3, the environment contains all the tools necessary to run and debug `supers`. For ergonomics, we use the tool [task](https://taskfile.dev) (included in the environment) to centralize the administrative tasks.

For example, to run `supers` with logs enabled and with potential for filtering, run

```
task run
```

See the full list of available tasks with `task -a`. Check documentation for a given task (e.g., `run`) with `task --summary run`.

## Configure

`supers` accept multiple formats for configuration: TOML, JSON, YAML, INI, RON, and JSON5.  It also accepts overriding the configuration via environment variables.

It reads the configuration from the following sources, in order. Each source can 
overwrite the values from the previous one. All sources are optional.

1. *Default configuration* (from the `Default` implementation for `ApplicationConfig`).
2. *User configuration file*:
   1. If the environment variable `SUPERS_CONF_FILE` is set, read the file taking its path from this variable. It is an error if the environment variable points to a non-existent file.
   2. If the environment variable `SUPERS_CONF_FILE` is **not** set, read the file `$CONFIG_DIR/supers/conf.{toml,yaml,yml,json,json5,ini,ron}`, where `$CONFIG_DIR` is the standard user's config directory for the platform (e.g., `~/.config` in Linux, `~/Library/Application Support` in macOS). `supers` ignore this source if the file does not exist.
3. *Environment variables*: an environment variable of the form `SUPERS_MY_FIELD` overwrites a field with name `my_field` in from the previous sources. 

## Endpoints

The `supers` administrative API provides the following endpoints:

Endpoint | Description 
-------- | -----------
`GET /ready` | Check that `supers` is running.
`GET /app` | Get the status of the application.
`GET /programs` | Get the status of all the programs defined in the application.
`GET /programs/{name}` | Get the status of the programs `{name}`.
`POST /programs/{name}/start` | Ensure that program `{name}` is running; i.e., start it if it is stopped.
`POST /programs/{name}/stop` | Ensure that program `{name}` is not running; i.e., stop it if it is running.
`POST /programs/{name}/restart` | Stop program `{name}` if it is running and then start it.

## Examples

1) Check the status of all programs: 

    ```bash
    $ curl localhost:8080/programs
    Program Statuses:
    sleep3: Running
    echo: Stopped
    ls: Stopped
    ```

2) Check the status of the `sleep3` program:

    ```bash
    $ curl localhost:8080/programs/sleep3
    Status of program sleep3 is: Running
    ```

3) Stop the `sleep3` program:

    ```bash
    $ curl localhost:8080/programs/sleep3/stop -X POST
    Program sleep3 has been instructed to stop.
    ```

4) Start the `sleep3` program:

    ```bash
    $ curl localhost:8080/programs/sleep3/start -X POST
    Program sleep3 has been instructed to start.
    ```