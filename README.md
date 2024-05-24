# OpenTSDB Auth Proxy

This is a simple proxy for the [OpenTSDB](https://github.com/OpenTSDB/opentsdb) 
time series database. It handles authentication and authorization.

This proxy can be publicly exposed. When sending data to opentsdb, set the endpoint
to this proxy instead. Each client will send the data alongside an authentication
token.

If the token matches the host and the metric matches the list of allowed metrics,
then the request is forwarded to the opentsdb server.

## Configuration

Take a look at the provided [sample configuration](./example-cfg.yml)

### Authentication tokens

Right now, two authentication tokens are supported:

- sha256
- plain (not recommended)

#### Sha256

To generate a sha256 token for a specific producer, do the following:

```bash
# Generate a token
TOKEN=$(openssl rand -hex 20)
SHA=$(echo -n $TOKEN | sha256sum - | awk '{print $1}')

echo "Token for the device is $TOKEN . Sha256 is $SHA"
# Token for the device is 7a5becc5b5bb581522fd0bb8891bb99a70275620 . Sha256 is ac790471b321143716e7773d589af923236ebdd435ba17c671df3558becc5154
```

The producer will need to send its token on query string:

```bash
curl -X POST https://my-proxy/api/put?token=7a5....
```

You then need to specify the hash in the config file. This file is then "safe"
if the token is reasonably random.

#### Plain

To be implemented; but don't do it.


## Environment variables

The following env variables are supported:

- `CONFIG_FILE` : the location of the config file
- `OPENTSDB_URL` (in this case, don't set it in the config file)

## Notes about exposing OpenTSDB

Currently, OpenTSDB does not support authentication. If you run opentsdb in a k8s
cluster, protect its ingress too. Either via a different ingress class, or with
specific per-ingress-ctrl anotations.
