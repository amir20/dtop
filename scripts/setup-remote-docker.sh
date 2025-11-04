#!/bin/bash
set -e

# Configuration
VM_NAME="${1:-docker-vm}"
DISTRO="${2:-ubuntu}"
CERT_DIR="$HOME/.docker/$VM_NAME"

echo "🚀 Setting up OrbStack VM: $VM_NAME with Docker TLS"

# Step 1: Create the VM
echo "📦 Creating VM..."
if orb list | grep -q "^$VM_NAME"; then
    echo "⚠️  VM $VM_NAME already exists. Delete it first with: orb delete $VM_NAME"
    exit 1
fi

orb create "$DISTRO" "$VM_NAME"
echo "✅ VM created"

# Wait for VM to be ready
sleep 3

# Step 2: Install Docker in the VM
echo "🐳 Installing Docker..."
if ! orb exec -m "$VM_NAME" bash -c '
curl -fsSL https://get.docker.com | sh
sudo usermod -aG docker $(whoami)
'; then
    echo "❌ Docker installation failed"
    exit 1
fi

echo "✅ Docker installed"

# Step 3: Generate TLS certificates in the VM
echo "🔐 Generating TLS certificates..."
if ! orb exec -m "$VM_NAME" bash -c '
set -e
mkdir -p ~/.docker

# Generate CA
echo "  Generating CA key and certificate..."
openssl genrsa -out ~/.docker/ca-key.pem 4096 2>/dev/null
openssl req -new -x509 -days 365 -key ~/.docker/ca-key.pem -sha256 \
    -out ~/.docker/ca.pem -subj "/CN=docker-ca" 2>/dev/null

# Get hostname
HOST=$(hostname)

# Generate server key and cert
echo "  Generating server key and certificate..."
openssl genrsa -out ~/.docker/server-key.pem 4096 2>/dev/null
openssl req -subj "/CN=$HOST" -sha256 -new -key ~/.docker/server-key.pem \
    -out ~/.docker/server.csr 2>/dev/null
echo "subjectAltName = DNS:$HOST,DNS:$HOST.orb.local,IP:127.0.0.1" > ~/.docker/extfile.cnf
openssl x509 -req -days 365 -sha256 -in ~/.docker/server.csr \
    -CA ~/.docker/ca.pem -CAkey ~/.docker/ca-key.pem -CAcreateserial \
    -out ~/.docker/server-cert.pem -extfile ~/.docker/extfile.cnf 2>/dev/null

# Generate client key and cert
echo "  Generating client key and certificate..."
openssl genrsa -out ~/.docker/key.pem 4096 2>/dev/null
openssl req -subj "/CN=client" -new -key ~/.docker/key.pem \
    -out ~/.docker/client.csr 2>/dev/null
echo "extendedKeyUsage = clientAuth" > ~/.docker/extfile-client.cnf
openssl x509 -req -days 365 -sha256 -in ~/.docker/client.csr \
    -CA ~/.docker/ca.pem -CAkey ~/.docker/ca-key.pem -CAcreateserial \
    -out ~/.docker/cert.pem -extfile ~/.docker/extfile-client.cnf 2>/dev/null

# Set permissions
chmod 0400 ~/.docker/ca-key.pem ~/.docker/key.pem ~/.docker/server-key.pem
chmod 0444 ~/.docker/ca.pem ~/.docker/server-cert.pem ~/.docker/cert.pem

# Cleanup
rm ~/.docker/*.csr ~/.docker/extfile*.cnf ~/.docker/ca.srl

# Verify files exist
echo "  Verifying certificate files..."
ls -la ~/.docker/*.pem
'; then
    echo "❌ Certificate generation failed"
    exit 1
fi

echo "✅ Certificates generated"

# Step 4: Configure Docker daemon for TLS
echo "⚙️  Configuring Docker daemon..."
if ! orb exec -m "$VM_NAME" bash -c '
set -e
USER=$(whoami)
sudo mkdir -p /etc/docker

sudo tee /etc/docker/daemon.json > /dev/null <<EOF
{
  "hosts": ["unix:///var/run/docker.sock", "tcp://0.0.0.0:2376"],
  "tls": true,
  "tlscacert": "/home/$USER/.docker/ca.pem",
  "tlscert": "/home/$USER/.docker/server-cert.pem",
  "tlskey": "/home/$USER/.docker/server-key.pem",
  "tlsverify": true
}
EOF

# Remove -H from systemd service if it exists (conflicts with daemon.json)
if [ -f /lib/systemd/system/docker.service ]; then
    sudo mkdir -p /etc/systemd/system/docker.service.d
    sudo tee /etc/systemd/system/docker.service.d/override.conf > /dev/null <<EOF
[Service]
ExecStart=
ExecStart=/usr/bin/dockerd
EOF
    sudo systemctl daemon-reload
fi

sudo systemctl restart docker
'; then
    echo "❌ Docker daemon configuration failed"
    exit 1
fi

echo "✅ Docker daemon configured"

# Step 5: Copy client certificates to host
echo "📋 Copying client certificates to host..."
mkdir -p "$CERT_DIR"

echo "  Copying ca.pem..."
orb exec -m "$VM_NAME" bash -c 'cat $HOME/.docker/ca.pem' > "$CERT_DIR/ca.pem"
echo "  Copying cert.pem..."
orb exec -m "$VM_NAME" bash -c 'cat $HOME/.docker/cert.pem' > "$CERT_DIR/cert.pem"
echo "  Copying key.pem..."
orb exec -m "$VM_NAME" bash -c 'cat $HOME/.docker/key.pem' > "$CERT_DIR/key.pem"

echo "  Setting permissions..."
chmod 0400 "$CERT_DIR/key.pem"
chmod 0444 "$CERT_DIR/ca.pem" "$CERT_DIR/cert.pem"

echo "✅ Client certificates copied to $CERT_DIR"

# Step 6: Test connection
echo "🧪 Testing connection..."
sleep 2
if docker --tlsverify \
    --tlscacert="$CERT_DIR/ca.pem" \
    --tlscert="$CERT_DIR/cert.pem" \
    --tlskey="$CERT_DIR/key.pem" \
    -H="$VM_NAME.orb.local:2376" info &>/dev/null; then
    echo "✅ Connection test successful!"
else
    echo "❌ Connection test failed. Check Docker daemon logs with: orb exec -m $VM_NAME sudo journalctl -u docker -n 50"
    exit 1
fi

# Print usage instructions
echo ""
echo "🎉 Setup complete!"
echo ""
echo "To connect to Docker on this VM, use:"
echo ""
echo "  docker --tlsverify \\"
echo "    --tlscacert=$CERT_DIR/ca.pem \\"
echo "    --tlscert=$CERT_DIR/cert.pem \\"
echo "    --tlskey=$CERT_DIR/key.pem \\"
echo "    -H=$VM_NAME.orb.local:2376 ps"
echo ""
echo "Or set environment variables:"
echo ""
echo "  export DOCKER_HOST=tcp://$VM_NAME.orb.local:2376"
echo "  export DOCKER_TLS_VERIFY=1"
echo "  export DOCKER_CERT_PATH=$CERT_DIR"
echo "  docker ps"
echo ""
echo "Add these to your ~/.bashrc or ~/.zshrc to make them permanent."
