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

echo "Building chat image..."
docker build -t chat:latest ./sensors-actuators/chat

echo "Building chat-front image..."
docker build -t chat-front:latest ./sensors-actuators/chat-front

# Apply Kubernetes configurations
echo "📦 Applying Kubernetes configurations..."
echo "Applying infer configurations..."
kubectl apply -f k8s/infer/

echo "Applying chat configurations..."
kubectl apply -f k8s/chat/

echo "Applying chat-front configurations..."
kubectl apply -f k8s/chat-front/

# Wait for pods to be ready
echo "⏳ Waiting for pods to be ready..."
kubectl wait --for=condition=ready pod -l app=infer --timeout=60s
kubectl wait --for=condition=ready pod -l app=chat --timeout=60s
kubectl wait --for=condition=ready pod -l app=chat-front --timeout=60s

echo "✅ Deployment complete!"
echo
echo "To check the status of your pods:"
echo "  kubectl get pods"
echo
echo "To test the chat service:"
echo "  kubectl port-forward service/chat 8080:8080"
echo
echo "To test the infer service:"
echo "  kubectl port-forward service/infer 8080:80"
echo
echo "To access the frontend:"
echo "  minikube service chat-front --url"