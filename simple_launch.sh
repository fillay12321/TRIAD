#!/bin/bash

# 🌐 Упрощенный запуск TRIAD Distributed Network
set -e

echo "🌐 Запуск TRIAD Distributed Network через GitHub Codespaces..."
echo "================================================================"

# Цвета для вывода
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

# Проверяем токен
if [ -z "${GITHUB_TOKEN:-}" ]; then
    echo -e "${RED}❌ GITHUB_TOKEN не установлен!${NC}"
    echo "Установите: export GITHUB_TOKEN=\$(gh auth token)"
    exit 1
fi

echo -e "${GREEN}✅ Токен установлен${NC}"

# Конфигурация
REPO_OWNER="fillay12321"
REPO_NAME="TRIAD"
BRANCH="main"
MACHINE="basicLinux32gb"
LOCATION="WestUs2"
TIMEOUT=30

# Создаем директорию для логов
mkdir -p logs

# Функция создания codespace
create_codespace() {
    local node_id="$1"
    local port="$2"
    local shard_id="$3"
    
    echo -e "${BLUE}🚀 Создаем Codespace для ${node_id}...${NC}"
    
    # Используем известный ID репозитория
    local repo_id="987166369"
    echo "ID репозитория: $repo_id"
    
    # Создаем JSON для codespace
    local create_json=$(cat <<EOF
{
    "repository_id": $repo_id,
    "machine": "$MACHINE",
    "location": "$LOCATION",
    "idle_timeout_minutes": $TIMEOUT,
    "display_name": "$node_id",
    "retention_period_minutes": 480,
    "ref": "$BRANCH"
}
EOF
)
    
    # Создаем codespace
    local response
    response=$(curl -s -X POST \
        -H "Authorization: token $GITHUB_TOKEN" \
        -H "Accept: application/vnd.github.v3+json" \
        -H "Content-Type: application/json" \
        -d "$create_json" \
        "https://api.github.com/user/codespaces")
    
    # Проверяем ответ
    local codespace_id
    codespace_id=$(echo "$response" | tr '\n' ' ' | grep -o '"id":[0-9]*' | cut -d':' -f2)
    
    # Проверяем, есть ли ошибка
    local error_msg=$(echo "$response" | tr '\n' ' ' | grep -o '"message":"[^"]*"' | cut -d'"' -f4)
    
    if [ -n "$error_msg" ]; then
        echo -e "${RED}❌ Ошибка создания codespace: $error_msg${NC}"
        return 1
    elif [ -n "$codespace_id" ]; then
        echo "$codespace_id" > "logs/${node_id}_codespace_id"
        echo -e "${GREEN}✅ Codespace создан: $codespace_id${NC}"
        return 0
    else
        echo -e "${RED}❌ Ошибка создания codespace${NC}"
        echo "Ответ: $response"
        return 1
    fi
}

# Создаем 10 codespaces
for i in {0..9}; do
    node_id="triad-node-$i"
    port=$((8080 + i))
    shard_id=$((i % 5))
    
    echo ""
    echo -e "${YELLOW}📋 Узел $i: ${node_id} (порт $port, shard $shard_id)${NC}"
    
    if create_codespace "$node_id" "$port" "$shard_id"; then
        echo -e "${GREEN}✅ ${node_id} создан успешно${NC}"
    else
        echo -e "${RED}❌ ${node_id} не удалось создать${NC}"
    fi
    
    sleep 2  # Пауза между созданием
done

echo ""
echo -e "${GREEN}🎉 Создание codespaces завершено!${NC}"
echo ""
echo -e "${BLUE}📊 Список созданных codespaces:${NC}"
ls -la logs/*_codespace_id 2>/dev/null || echo "Нет созданных codespaces"
