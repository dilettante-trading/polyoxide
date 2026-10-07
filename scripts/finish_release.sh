#!/bin/bash
set -e

# Polyoxide Release Recovery Script
# Use this to finish publishing crates if the main release process failed.

echo "🚀 Starting manual release..."

echo "📦 Publishing polyoxide-core..."
cargo publish -p polyoxide-core
echo "✅ polyoxide-core published"

echo "📦 Publishing polyoxide-rtds..."
cargo publish -p polyoxide-rtds
echo "✅ polyoxide-rtds published"

echo "📦 Publishing polyoxide-sports..."
cargo publish -p polyoxide-sports
echo "✅ polyoxide-sports published"

echo "📦 Publishing polyoxide-perps..."
cargo publish -p polyoxide-perps
echo "✅ polyoxide-perps published"

echo "📦 Publishing polyoxide-binance..."
cargo publish -p polyoxide-binance
echo "✅ polyoxide-binance published"

echo "📦 Publishing polyoxide-relay..."
cargo publish -p polyoxide-relay
echo "✅ polyoxide-relay published"

echo "📦 Publishing polyoxide-gamma..."
cargo publish -p polyoxide-gamma
echo "✅ polyoxide-gamma published"

echo "📦 Publishing polyoxide-data..."
cargo publish -p polyoxide-data
echo "✅ polyoxide-data published"

# Wait for index propagation
echo "⏳ Waiting 30s for index propagation..."
sleep 30

echo "📦 Publishing polyoxide-clob..."
cargo publish -p polyoxide-clob
echo "✅ polyoxide-clob published"

# Wait for index propagation
echo "⏳ Waiting 20s for index propagation..."
sleep 20

echo "📦 Publishing polyoxide..."
cargo publish -p polyoxide
echo "✅ polyoxide published"

echo "📦 Publishing polyoxide-cli..."
cargo publish -p polyoxide-cli
echo "✅ polyoxide-cli published"

echo "🎉 Release recovery complete!"
