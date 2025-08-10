#!/bin/bash

echo "Тестируем токен..."

# Проверяем токен напрямую
response=$(curl -s -H "Authorization: token $GITHUB_TOKEN" -H "Accept: application/vnd.github.v3+json" "https://api.github.com/user")
echo "Ответ получен, длина: ${#response}"

# Извлекаем имя пользователя
username=$(echo "$response" | grep -o '"login":"[^"]*"' | cut -d'"' -f4)
echo "Имя пользователя: '$username'"

if [ -n "$username" ]; then
    echo "✅ Токен работает для пользователя: $username"
else
    echo "❌ Не удалось извлечь имя пользователя"
    echo "Ответ: $response"
fi
