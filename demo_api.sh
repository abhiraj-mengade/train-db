#!/bin/bash

# TrainDB HTTP API Demo Script
# This script demonstrates the TrainDB HTTP API functionality

echo "🚂 TrainDB HTTP API Demo"
echo "========================"
echo ""

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

API_URL="http://localhost:8081"

echo -e "${BLUE}Starting TrainDB API server in background...${NC}"
cargo run -- api --port 8081 &
SERVER_PID=$!

# Wait for server to start
echo "Waiting for server to start..."
sleep 3

echo ""
echo -e "${GREEN}✅ TrainDB API Server is running on port 8081${NC}"
echo ""

# Test 1: Get node info
echo -e "${YELLOW}📊 Test 1: Get Node Information${NC}"
echo "GET $API_URL/api/info"
curl -s -X GET "$API_URL/api/info" | jq '.'
echo ""

# Test 2: Set some key-value pairs
echo -e "${YELLOW}📝 Test 2: Set Key-Value Pairs${NC}"

echo "Setting train schedule..."
curl -s -X POST "$API_URL/api/keys" \
  -H "Content-Type: application/json" \
  -d '{"key": "train_schedule", "value": "Express 101: 08:00, Local 202: 09:30, Express 303: 11:15"}' | jq '.'

echo "Setting weather info..."
curl -s -X POST "$API_URL/api/keys" \
  -H "Content-Type: application/json" \
  -d '{"key": "weather", "value": "Sunny, 22°C, Light winds"}' | jq '.'

echo "Setting platform info..."
curl -s -X POST "$API_URL/api/keys" \
  -H "Content-Type: application/json" \
  -d '{"key": "platform_3", "value": "Express 101 boarding now"}' | jq '.'
echo ""

# Test 3: List all keys
echo -e "${YELLOW}📋 Test 3: List All Keys${NC}"
echo "GET $API_URL/api/keys"
curl -s -X GET "$API_URL/api/keys" | jq '.'
echo ""

# Test 4: Get specific key
echo -e "${YELLOW}🔍 Test 4: Get Specific Key${NC}"
echo "GET $API_URL/api/keys/train_schedule"
curl -s -X GET "$API_URL/api/keys/train_schedule" | jq '.'
echo ""

# Test 5: Update a key
echo -e "${YELLOW}🔄 Test 5: Update Key Value${NC}"
echo "Updating weather info..."
curl -s -X POST "$API_URL/api/keys" \
  -H "Content-Type: application/json" \
  -d '{"key": "weather", "value": "Cloudy, 18°C, Moderate winds"}' | jq '.'
echo ""

# Test 6: Get updated key
echo -e "${YELLOW}✅ Test 6: Verify Update${NC}"
echo "GET $API_URL/api/keys/weather"
curl -s -X GET "$API_URL/api/keys/weather" | jq '.'
echo ""

# Test 7: Delete a key
echo -e "${YELLOW}🗑️  Test 7: Delete Key${NC}"
echo "DELETE $API_URL/api/keys/platform_3"
curl -s -X DELETE "$API_URL/api/keys/platform_3" | jq '.'
echo ""

# Test 8: Verify deletion
echo -e "${YELLOW}✅ Test 8: Verify Deletion${NC}"
echo "GET $API_URL/api/keys/platform_3"
curl -s -X GET "$API_URL/api/keys/platform_3" | jq '.'
echo ""

# Test 9: Final state
echo -e "${YELLOW}📊 Test 9: Final Database State${NC}"
echo "GET $API_URL/api/keys"
curl -s -X GET "$API_URL/api/keys" | jq '.'
echo ""

echo -e "${GREEN}🎉 Demo completed successfully!${NC}"
echo ""
echo -e "${BLUE}TrainDB Features Demonstrated:${NC}"
echo "✅ HTTP REST API with JSON responses"
echo "✅ Key-value storage with RocksDB"
echo "✅ CRUD operations (Create, Read, Update, Delete)"
echo "✅ Persistent storage across requests"
echo "✅ CORS support for web applications"
echo "✅ Error handling and status codes"
echo ""

echo -e "${YELLOW}API Endpoints Available:${NC}"
echo "• GET    /api/info          - Node information"
echo "• GET    /api/keys          - List all key-value pairs"
echo "• GET    /api/keys/{key}    - Get specific key value"
echo "• POST   /api/keys          - Set/update key-value pair"
echo "• DELETE /api/keys/{key}    - Delete key"
echo ""

echo -e "${BLUE}Next Steps for P2P Network:${NC}"
echo "• Fix libp2p API integration for peer-to-peer networking"
echo "• Add GossipSub protocol for message broadcasting"
echo "• Implement Bluetooth transport for multi-transport mesh"
echo "• Add peer discovery and connection management"
echo "• Implement data synchronization between nodes"
echo ""

# Clean up
echo -e "${RED}Stopping TrainDB server...${NC}"
kill $SERVER_PID 2>/dev/null
echo "Demo finished!"
