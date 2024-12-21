#!/bin/bash

# Exit on any error
set -e

echo "🚀 Starting deployment process..."

# Ensure Minikube is running
if ! minikube status > /dev/null 2>&1; then
  echo "Starting Minikube..."
  minikube start
fi

# Point to Minikube's Docker daemon
echo "🔄 Configuring Docker environment..."
eval $(minikube docker-env)

# Build images
echo "🏗️  Building Docker images..."
echo "Building infer image..."
docker build -t infer:latest ./infer

echo "Building web-chat image..."
docker build -t web-chat:latest ./sensors-actuators/web-chat

# Apply Kubernetes configurations
echo "📦 Applying Kubernetes configurations..."
echo "Applying infer configurations..."
kubectl apply -f k8s/infer/

echo "Applying web-chat configurations..."
kubectl apply -f k8s/web-chat/

# Wait for pods to be ready
echo "⏳ Waiting for pods to be ready..."
kubectl wait --for=condition=ready pod -l app=infer --timeout=60s
kubectl wait --for=condition=ready pod -l app=web-chat --timeout=60s

echo "✅ Deployment complete!"
echo
echo "To check the status of your pods:"
echo "  kubectl get pods"
echo
echo "To test the web-chat service:"
echo "  kubectl port-forward service/web-chat 8080:8080"
echo
echo "To test the infer service:"
echo "  kubectl port-forward service/infer 8080:80" 