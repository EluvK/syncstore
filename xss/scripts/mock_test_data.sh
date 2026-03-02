#!/bin/bash

# 配置参数
BASE_URL="http://127.0.0.1:10101/api" # 请根据实际修改
USERNAME="mock"
PASSWORD="test"
NAMESPACE="xbb"

# 1. 登录获取 Token
echo "Logging in..."
LOGIN_RES=$(curl -s -X POST "$BASE_URL/auth/name-login" \
  -H "Content-Type: application/json" \
  -d "{\"username\":\"$USERNAME\", \"password\":\"$PASSWORD\"}")
echo "Login response: $LOGIN_RES"
# 使用 sed 正则提取 access_token 的值
TOKEN=$(echo "$LOGIN_RES" | sed -n 's/.*"access_token":"\([^"]*\)".*/\1/p')

if [ "$TOKEN" == "null" ] || [ -z "$TOKEN" ]; then
  echo "Login failed"
  exit 1
fi

echo "Login successful. Token acquired."

# 循环创建 5 个 Repo
for i in {2..5}; do
  REPO_NAME="Repo_$i"
  echo "Creating Repo: $REPO_NAME"
  
  REPO_ID=$(curl -s -X POST "$BASE_URL/data/$NAMESPACE/repo" \
    -H "Authorization: Bearer $TOKEN" \
    -H "Content-Type: application/json" \
    -d "{\"name\": \"$REPO_NAME\", \"status\": \"normal\", \"description\": \"Description for $REPO_NAME\"}")
  
  # 去掉返回 ID 可能存在的引号
  REPO_ID=$(echo $REPO_ID | tr -d '"')

  # 每个 Repo 创建 3 个 Category
  for j in {1..3}; do
    CAT_NAME="Category_$j"
    
    # 每个 Category 创建 10 个 Post
    for k in {1..10}; do
      echo "  Creating Post $k in $CAT_NAME (Repo $i)"
      
      POST_ID=$(curl -s -X POST "$BASE_URL/data/$NAMESPACE/post" \
        -H "Authorization: Bearer $TOKEN" \
        -H "Content-Type: application/json" \
        -d "{
          \"title\": \"Post Title $k\",
          \"category\": \"$CAT_NAME\",
          \"content\": \"This is the content for post $k under $CAT_NAME\",
          \"repo_id\": \"$REPO_ID\",
          \"parent_id\": \"$REPO_ID\"
        }")
      
      POST_ID=$(echo $POST_ID | tr -d '"')

      # 每个 Post 创建 3 个 Comment
      for l in {1..3}; do
        curl -s -X POST "$BASE_URL/data/$NAMESPACE/comment" \
          -H "Authorization: Bearer $TOKEN" \
          -H "Content-Type: application/json" \
          -d "{
            \"content\": \"Comment $l for post $POST_ID\",
            \"post_id\": \"$POST_ID\",
            \"parent_id\": \"$POST_ID\"
          }" > /dev/null
      done
    done
  done
done

echo "Mock data creation completed!"