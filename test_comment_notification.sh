#!/bin/bash
# Test script to verify WebSocket notifications for comment replies

echo "🔍 Testing WebSocket notifications for comment replies..."

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Set JWT_SECRET to match what the server is using
export JWT_SECRET="your_development_secret_key"

# Step 1: Check if server is running
if ! lsof -i:9500 >/dev/null 2>&1; then
    echo -e "${RED}❌ Server is not running on port 9500. Please start the server before running tests.${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Server is running on port 9500.${NC}"

# Step 2: Authenticate as first user (a@a.com)
echo -e "${BLUE}🔑 Authenticating as first user (a@a.com)...${NC}"

USER1_AUTH_RESPONSE=$(curl -s -X POST http://localhost:9500/api/auth/login \
    -H "Content-Type: application/json" \
    -d '{"email":"a@a.com","password":"123"}')

USER1_ACCESS_TOKEN=$(echo "$USER1_AUTH_RESPONSE" | grep -o '"token":"[^"]*' | sed 's/"token":"//')
USER1_ID=$(echo "$USER1_AUTH_RESPONSE" | grep -o '"user_id":"[^"]*' | sed 's/"user_id":"//')

if [ -z "$USER1_ACCESS_TOKEN" ]; then
    echo -e "${RED}❌ Authentication failed for first user. Could not retrieve access token.${NC}"
    echo -e "${YELLOW}Response: $USER1_AUTH_RESPONSE${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Successfully authenticated as first user. Token retrieved.${NC}"
echo -e "${GREEN}✅ User 1 ID: $USER1_ID${NC}"

# Step 3: Authenticate as second user (b@b.com)
echo -e "${BLUE}🔑 Authenticating as second user (b@b.com)...${NC}"

USER2_AUTH_RESPONSE=$(curl -s -X POST http://localhost:9500/api/auth/login \
    -H "Content-Type: application/json" \
    -d '{"email":"b@b.com","password":"123"}')

USER2_ACCESS_TOKEN=$(echo "$USER2_AUTH_RESPONSE" | grep -o '"token":"[^"]*' | sed 's/"token":"//')
USER2_ID=$(echo "$USER2_AUTH_RESPONSE" | grep -o '"user_id":"[^"]*' | sed 's/"user_id":"//')

if [ -z "$USER2_ACCESS_TOKEN" ]; then
    echo -e "${RED}❌ Authentication failed for second user. Could not retrieve access token.${NC}"
    echo -e "${YELLOW}Response: $USER2_AUTH_RESPONSE${NC}"
    exit 1
fi

echo -e "${GREEN}✅ Successfully authenticated as second user. Token retrieved.${NC}"
echo -e "${GREEN}✅ User 2 ID: $USER2_ID${NC}"

# Step 4: First user posts a comment to post ID 5
echo -e "${BLUE}💬 User 1 posting a comment to post ID 5...${NC}"

TIMESTAMP=$(date +%s)
USER1_COMMENT_RESPONSE=$(curl -s -X POST http://localhost:9500/api/posts/5/comments \
    -H "Content-Type: application/json" \
    -H "Authorization: Bearer $USER1_ACCESS_TOKEN" \
    -d "{\"content\":\"Initial comment from User 1 at $TIMESTAMP\", \"parent_comment_id\": null, \"markdown_enabled\": false}")

USER1_COMMENT_ID=$(echo "$USER1_COMMENT_RESPONSE" | grep -o '"id":[0-9]*' | head -1 | sed 's/"id"://')

if [ -z "$USER1_COMMENT_ID" ]; then
    echo -e "${RED}❌ First user's comment posting failed.${NC}"
    echo -e "${YELLOW}Response: $USER1_COMMENT_RESPONSE${NC}"
    exit 1
fi

echo -e "${GREEN}✅ User 1 comment posted successfully with ID: $USER1_COMMENT_ID${NC}"

# Step 5: Create a basic HTML file with WebSocket client for User 1
WS_TEST_FILE=$(mktemp)
cat > $WS_TEST_FILE << EOF
<!DOCTYPE html>
<html>
<head>
    <title>WebSocket Notification Test</title>
    <style>
        body { font-family: Arial, sans-serif; margin: 20px; }
        .container { display: flex; }
        .box { flex: 1; margin: 10px; padding: 15px; border: 1px solid #ccc; border-radius: 5px; }
        .notification { padding: 10px; margin: 5px 0; border-left: 4px solid #4CAF50; background-color: #f9f9f9; }
        h2 { color: #333; }
        .user-details { margin-bottom: 20px; }
        .status { padding: 10px; margin: 10px 0; border-radius: 5px; }
        .connected { background-color: #D4EDDA; color: #155724; }
        .waiting { background-color: #FFF3CD; color: #856404; }
        .disconnected { background-color: #F8D7DA; color: #721C24; }
        button { margin-top: 10px; padding: 8px 15px; background-color: #4CAF50; color: white; border: none; border-radius: 4px; cursor: pointer; }
        button:hover { background-color: #45a049; }
    </style>
</head>
<body>
    <h1>WebSocket Notification Test</h1>
    <div class="container">
        <div class="box">
            <h2>User 1 (Original Comment Author)</h2>
            <div class="user-details">
                <p><strong>User ID:</strong> ${USER1_ID}</p>
                <p><strong>Comment ID:</strong> ${USER1_COMMENT_ID}</p>
                <p><strong>Token:</strong> ${USER1_ACCESS_TOKEN}</p>
            </div>
            <div id="user1-status" class="status waiting">Waiting to connect...</div>
            <button onclick="connectUser1()">Connect User 1 WebSocket</button>
            <h3>Notifications:</h3>
            <div id="user1-notifications"></div>
        </div>
        <div class="box">
            <h2>User 2 (Reply Author)</h2>
            <div class="user-details">
                <p><strong>User ID:</strong> ${USER2_ID}</p>
                <p><strong>Token:</strong> ${USER2_ACCESS_TOKEN}</p>
            </div>
            <div id="user2-status" class="status waiting">Waiting to connect...</div>
            <button onclick="connectUser2()">Connect User 2 WebSocket</button>
            <h3>Actions:</h3>
            <button onclick="postReply()">Post Reply to User 1's Comment</button>
            <div id="reply-status"></div>
        </div>
    </div>
    <script>
        let user1WS = null;
        let user2WS = null;
        
        function updateStatus(id, message, type) {
            const statusDiv = document.getElementById(id);
            statusDiv.innerHTML = message;
            statusDiv.className = 'status ' + type;
        }
        
        function addNotification(userId, message) {
            const notificationsDiv = document.getElementById(userId + '-notifications');
            const notifDiv = document.createElement('div');
            notifDiv.className = 'notification';
            notifDiv.innerHTML = '<strong>' + new Date().toLocaleTimeString() + ':</strong> ' + message;
            notificationsDiv.appendChild(notifDiv);
        }

        function connectUser1() {
            updateStatus('user1-status', 'Connecting...', 'waiting');
            
            // Close previous connection if exists
            if (user1WS) {
                user1WS.close();
            }
            
            // Connect to the WebSocket server
            user1WS = new WebSocket('ws://localhost:9500/api/notifications/ws?token=${USER1_ACCESS_TOKEN}');
            
            user1WS.onopen = () => {
                updateStatus('user1-status', '✅ Connected to WebSocket server', 'connected');
                addNotification('user1', 'WebSocket connection established');
            };
            
            user1WS.onmessage = (event) => {
                const message = 'Received notification: ' + event.data;
                addNotification('user1', message);
                try {
                    const notificationData = JSON.parse(event.data);
                    if (notificationData.type === 'comment_reply') {
                        updateStatus('user1-status', '✅ Reply notification received!', 'connected');
                    }
                } catch (e) {
                    console.error('Error parsing notification:', e);
                }
            };
            
            user1WS.onerror = (error) => {
                addNotification('user1', 'Error: ' + JSON.stringify(error));
                updateStatus('user1-status', '❌ WebSocket error', 'disconnected');
            };
            
            user1WS.onclose = () => {
                addNotification('user1', 'WebSocket connection closed');
                updateStatus('user1-status', '⚠️ WebSocket connection closed', 'disconnected');
            };
        }

        function connectUser2() {
            updateStatus('user2-status', 'Connecting...', 'waiting');
            
            // Close previous connection if exists
            if (user2WS) {
                user2WS.close();
            }
            
            // Connect to the WebSocket server
            user2WS = new WebSocket('ws://localhost:9500/api/notifications/ws?token=${USER2_ACCESS_TOKEN}');
            
            user2WS.onopen = () => {
                updateStatus('user2-status', '✅ Connected to WebSocket server', 'connected');
            };
            
            user2WS.onmessage = (event) => {
                console.log('User 2 received:', event.data);
            };
            
            user2WS.onerror = (error) => {
                console.error('User 2 error:', error);
                updateStatus('user2-status', '❌ WebSocket error', 'disconnected');
            };
            
            user2WS.onclose = () => {
                updateStatus('user2-status', '⚠️ WebSocket connection closed', 'disconnected');
            };
        }

        function postReply() {
            const replyStatusDiv = document.getElementById('reply-status');
            replyStatusDiv.innerHTML = 'Posting reply...';
            
            const timestamp = Date.now();
            fetch('http://localhost:9500/api/posts/5/comments', {
                method: 'POST',
                headers: {
                    'Content-Type': 'application/json',
                    'Authorization': 'Bearer ${USER2_ACCESS_TOKEN}'
                },
                body: JSON.stringify({
                    content: 'Reply from User 2 at ' + timestamp,
                    parent_comment_id: ${USER1_COMMENT_ID},
                    markdown_enabled: false
                })
            })
            .then(response => response.json())
            .then(data => {
                replyStatusDiv.innerHTML = '✅ Reply posted successfully! ID: ' + data.id;
                console.log('Reply posted:', data);
            })
            .catch(error => {
                replyStatusDiv.innerHTML = '❌ Error posting reply: ' + error.message;
                console.error('Error posting reply:', error);
            });
        }

        // Auto-connect both users after 1 second
        setTimeout(() => {
            connectUser1();
            setTimeout(() => connectUser2(), 1000); // stagger connections
        }, 1000);
    </script>
</body>
</html>
EOF

echo -e "${BLUE}Created interactive WebSocket test file at $WS_TEST_FILE${NC}"
echo -e "${YELLOW}To test WebSocket notifications, open this file in a browser:${NC}"
echo -e "${YELLOW}file://$WS_TEST_FILE${NC}"

echo -e "${GREEN}✅ WebSocket notification test setup completed!${NC}"
echo -e "${YELLOW}Instructions:${NC}"
echo -e "${YELLOW}1. Open the HTML file in your browser${NC}"
echo -e "${YELLOW}2. Both users will automatically connect to the WebSocket${NC}"
echo -e "${YELLOW}3. Click the 'Post Reply' button to have User 2 reply to User 1's comment${NC}"
echo -e "${YELLOW}4. User 1 should receive a notification for the reply${NC}"

echo -e "${YELLOW}Don't forget to unset JWT_SECRET when done:${NC}"
echo -e "${YELLOW}unset JWT_SECRET${NC}"
