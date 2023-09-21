 <p align="center">
    <img src="docs/header.png" alt="defguard">
 </p>

[Defguard](https://github.com/DefGuard/defguard) provisioning client for Yubikeys module.


## Supported platforms

- Linux
- Windows
- MacOS

## Prerequisites
Machine using this service needs to have proper drivers/tools to detect and operate on smartcard's.
While macOS and Windows should work without installing additional software, on Linux make sure your distribution has all the necessary tools to detect, read, and write on smartcard's.

Also, the following tools are **required**:
- [ykman](https://developers.yubico.com/yubikey-manager/)
- gpg version 2

## Configuration
The following information is **required** in order to launch client, these can be configured by supplying correct arguments or setting corresponding environment variables. For additional configuration options check **--help**.

| Name                 | Description                                                                            | Environment variable | Argument |
|----------------------|----------------------------------------------------------------------------------------|----------------------|----------|
|          ID          |                   Used to identify client, this is showed in main UI.                  |        **ID**        |   --id   |
|   GRPC endpoint URL  |                   This needs to point to active Defguard GRPC server.                  |     **GRPC_URL**     |  --grpc  |
| Authentication token | Token to authorize client, this can be found in provisioners page in main Defguard UI. |       **TOKEN**      |  --token |

## Docker
This tool can also be used from a docker image like so:
```bash
docker run --privileged ghcr.io/defguard/yubikey-provision:main -t <TOKEN> --id <ID> --grpc <DEFGUARD_GRPC_URL>
```
Note that image is using elevated privileges to access host's USB by default but you can also try to configure it with **--device**.

## macOS

```
brew install rust ykman gpg2 protobuf
cargo build
./target/debug/yubikey-provision --id <id> --token <token_from_defguard> --grpc "defguard-grpc.host.name"
```

## Documentation

See the [documentation](https://defguard.gitbook.io) for more information.

## Community and Support

Find us on Matrix: [#defguard:teonite.com](https://matrix.to/#/#defguard:teonite.com)

## Contribution

Please review the [Contributing guide](https://defguard.gitbook.io/defguard/for-developers/contributing) for information on how to get started contributing to the project. You might also find our [environment setup guide](https://defguard.gitbook.io/defguard/for-developers/dev-env-setup) handy.
