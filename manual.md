# Getting started
Cli for (gameserver-rs)[https://github.com/SpiderUnderUrBed/gameserver-rs]

Here is how to use the cli to interact with the gameserver node

You can get alot of info from `--help`, especially for all subcommands,
I did not extensively describe what values are needed to be put into there, so this
manual is a substitution for that

**note, it sends all the outputs as yaml, but do not be confused, the server does not operate with yaml, its only formatted that way to look nice**

## command-settings, local config stored in db.json, and controls how the cli connects to the server and some extra things
`set` will save the connection details among some other things, 
takes `--url`, `--auth-token`, `--external-process-pid`, `--external-process-systemd`, `--forward-actions-url` and `--forward-actions`, all of these flags are optional
`--url` will set the url of the panel/webserver which manages the gameserver node and also serves a UI, in this case its being used 
to just call its api to manage it
`--auth-token` HAS to coorospond with the value of `HEADER_TOKEN=<TOKEN HERE>` set for the gameserver panel, its how it 
will be authorized to do ANYTHING, given admin perms with the scope all

`get` this will print the current saved config

As of the time of writing this, `--forward-actions` and `--forward-actions-url` is completely not implimented and `--external-process-pid` and `--external-process-systemd` is partially not implimented (you can still see it in the database and use it)
Its a bit of a niche feature, but it can and should be used to keep track of processes running **a** gameserver
if not using the main panel, incase there are people who does not have access to the server this is running on, 
someone could impliment something to let people know the state of the gameserver, even in its current state its recommended if you are
not using the system, still tell it via `--external-process-pid` or `--external-process-systemd`

## settings, the gameservers backend settings
`set` will fetch the current settings from the server, and merge your arguments ontop (consult --help for settings subcommand to see whats avalible).
you have `--rcon-url`, `--rcon-password`, `--enabled-rcon`, `--filter`, `--status-type`, `--toggled-default-buttons`, `--enable-statistics-on-home-page`, `--enable-nodes-on-home-page`, `--console-entry-on-top`, `--file-system-driver`, `--current-server`

`get` fetches and prints the current server settings

## server, manages the game servers
`create` will create a new server, the options are `--servername`, `--provider`, `--providertype`, `--location`, `--sandbox`, `--node` (JSON string), `--server-metadata` (JSON string). `--node` and `--server-metadata` is not required
`get` lists all servers
`start` sends a start command to the currently selected/active server
`stop` sends the stop command to the currently selected/active server.
`set --servername <name>` switches the active server the backend in managing
`delete --servername <name> --delete-files`, `--delete-files` is optional, defaults to false.

## nodes, manages the nodes
`create`, creates a node. Options: `--nodename`, `--ip`, `--nodestatus`, `--nodetype`, `--k8s-type`. On bare metal configurations or in certain cases you can just ignore k8s type
`--nodetype` and `--nodestatus` also has a default value and can be left out
`get` lists all nodes

## users, manages users
`create`, creates a user, options `--username`, `--password`, `--user-perms` (comma seperated, like admin:all, coorosponding to role:scope)
`get` lists all users

## state, shows or changes what the backend is pointed at
`get`, prints the current active server and current active node.
`set --node-id <id> --server-id <id>`, if both `--node-id` and `--server-id` flags are specified, within the state 
it will change the current node the backend is pointed at, `id` is simply the name of the server or node

## stream, connects to the backend websocket
`follow` read-only and it prints everything running in the server broadcast
`interact` bidirectional, shows a > prompt, you type commands that get forwarded over the websocket to the server process, `/quit` to exit
adding the `--no-duplicates` flag to either command will supress the same message happening in a streak
adding `--no-json` will try to suppress json output
adding `--output-systemd <node service name>` will get its output from the systemd service
ideally the one actually running the node, incase the websocket output is unreliable
 
All commands except command-settings use the url and auth-token saved via command-settings set.
