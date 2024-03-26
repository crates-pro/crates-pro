#!/bin/bash


# write contents into lgraph_daemon.json 
cat << EOF > /build/output/lgraph_daemon.json
{
  "directory": "/var/lib/lgraph/data",
  "host": "0.0.0.0",
  "port": 7070,
  "enable_rpc": false,
  "rpc_port": 9090,
  "verbose": 1,
  "log_dir": "/var/log/lgraph_log",
  "ssl_auth": false,
  "server_key": "/usr/local/etc/lgraph/server-key.pem",
  "server_cert": "/usr/local/etc/lgraph/server-cert.pem",
  "bolt_port": 7687
}
EOF

echo "Confiuration written in /build/output/lgraph_daemon.json"

/build/output/lgraph_server -c /build/output/lgraph_daemon.json -d start