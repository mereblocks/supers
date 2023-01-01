# `supers`
Programmable supervisor for long-running programs. In `supers`, an application is defined as a set of programs with
restart policies. `supers` reads the application's configuration of programs and starts them up. Then, it presents a basic
administrative API over HTTP that can be used to control the status of the running programs.

## Build and Run

Currently, the application config lives in the rust code (see the function `get_test_app_config`). We will update this in the future to read a TOML config file. For now, change the function to modify the programs run by `supers`.

Then, build the application with cargo and run the resulting binary. 

```
cargo build
./supers
```

The `supers` administrative API listens on port 8080.


## Configure
Create a toml file either in the default path (`/etc/supers/conf.toml`) or specify a custom path using the `SUPERS_CONF_FILE`
enironment variable, e.g., 

```
$ export SUPERS_CONF_FILE=/home/jstubbs/gits/mereblocks/supers/ex_config.toml
```


## Endpoints

The `supers` administrative API provides the following endpoints:

```
GET /ready -- Check that supers is running

GET /app -- Get the status of the application

GET /programs -- Get the status of all the programs defined in the application.

GET  /programs/{name} -- Get the status of the programs {name}.

POST /programs/{name}/start - Ensure that program {name} is running; i.e., start it if it is stopped.

POST /programs/{name}/stop - Ensure that program {name} is not running; i.e., stop it if it is running.

POST /programs/{name}/restart - Stop program {name} if is running and then start it.
```

## Examples

1) Check the status of all programs: 

```
$ curl localhost:8080/programs
Program Statuses:
sleep3: Running
echo: Stopped
ls: Stopped
```

2) Check the status of the sleep3 program:

```
$ curl localhost:8080/programs/sleep3
Status of program sleep3 is: Running
```

3) Stop the sleep3 program:

```
$ curl localhost:8080/programs/sleep3/stop -X POST
Program sleep3 has been instructed to stop.
```

4) Start the sleep3 program:
```
$ curl localhost:8080/programs/sleep3/start -X POST
Program sleep3 has been instructed to start.
```