#!/bin/bash

# 🔧 GitHub API Configuration for TRIAD Codespaces
# Конфигурация для управления GitHub Codespaces через API

# Включаем строгий режим
set -euo pipefail

# GitHub API настройки
export GITHUB_API_VERSION="2022-11-28"
export GITHUB_API_BASE="https://api.github.com"

# GitHub Codespaces настройки
export CODESPACE_MACHINE="basicLinux32gb"  # Размер машины
export CODESPACE_LOCATION="WestUs2"         # Локация (WestUs2, EastUs, WestEurope, SoutheastAsia)
export CODESPACE_IDLE_TIMEOUT="30"          # Таймаут бездействия в минутах

# TRIAD Network конфигурация
export TRIAD_REPO_OWNER="fillay12321"           # GitHub username
export TRIAD_REPO_NAME="TRIAD"                   # Имя репозитория
export TRIAD_BRANCH="main"                       # Ветка для Codespaces

# Codespaces конфигурация
export CODESPACE_COUNT=10                       # Количество Codespaces
export CODESPACE_PREFIX="triad-node"            # Префикс для имен
export CODESPACE_START_PORT=8080                # Начальный порт

# GitHub Personal Access Token (должен быть установлен в переменной окружения)
# export GITHUB_TOKEN="your-token-here"

# Функция для установки токена из файла или переменной окружения
setup_github_token() {
    # Сначала проверяем переменную окружения
    if [ -n "${GITHUB_TOKEN:-}" ]; then
        log_info "Используем токен из переменной окружения GITHUB_TOKEN"
        return 0
    fi
    
    # Проверяем файл с токеном
    local token_file="$HOME/.github_token"
    if [ -f "$token_file" ]; then
        log_info "Загружаем токен из файла $token_file"
        export GITHUB_TOKEN=$(cat "$token_file" | tr -d '\n\r')
        return 0
    fi
    
    # Проверяем файл .env в текущей директории
    if [ -f ".env" ]; then
        log_info "Загружаем токен из файла .env"
        export GITHUB_TOKEN=$(grep "^GITHUB_TOKEN=" .env | cut -d'=' -f2- | tr -d '\n\r')
        if [ -n "$GITHUB_TOKEN" ]; then
            return 0
        fi
    fi
    
    # Проверяем GitHub CLI
    if command -v gh &> /dev/null; then
        log_info "Пытаемся получить токен через GitHub CLI..."
        if gh auth status &> /dev/null; then
            export GITHUB_TOKEN=$(gh auth token)
            if [ -n "$GITHUB_TOKEN" ]; then
                log_success "Токен получен через GitHub CLI"
                return 0
            fi
        fi
    fi
    
    return 1
}

# Цвета для вывода
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Функция логирования
log_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

log_debug() {
    if [ "${DEBUG:-false}" = "true" ]; then
        echo -e "${PURPLE}🔍 $1${NC}"
    fi
}

# Проверка наличия необходимых утилит
check_dependencies() {
    local missing_deps=()
    
    if ! command -v curl &> /dev/null; then
        missing_deps+=("curl")
    fi
    
    if ! command -v jq &> /dev/null; then
        missing_deps+=("jq")
    fi
    
    if [ ${#missing_deps[@]} -gt 0 ]; then
        log_error "Отсутствуют необходимые зависимости: ${missing_deps[*]}"
        log_info "Установите их с помощью:"
        echo "  Ubuntu/Debian: sudo apt-get install curl jq"
        echo "  macOS: brew install curl jq"
        echo "  CentOS/RHEL: sudo yum install curl jq"
        exit 1
    fi
    
    log_success "Все зависимости установлены"
}

# Проверка наличия токена
check_github_token() {
    # Пытаемся установить токен
    if ! setup_github_token; then
        log_error "GITHUB_TOKEN не найден!"
        log_info "Установите токен одним из способов:"
        echo "  1. Экспорт: export GITHUB_TOKEN='your-token-here'"
        echo "  2. Файл: echo 'your-token-here' > ~/.github_token"
        echo "  3. .env файл: echo 'GITHUB_TOKEN=your-token-here' > .env"
        echo "  4. GitHub CLI: gh auth login"
        echo ""
        log_info "Токен должен иметь права:"
        echo "  - repo (полный доступ к репозиториям)"
        echo "  - codespace (управление Codespaces)"
        echo "  - workflow (запуск Actions)"
        exit 1
    fi
    
    # Проверяем валидность токена
    log_info "Проверяем валидность GitHub токена..."
    
    # Простой способ проверки токена
    local response
    response=$(curl -s -H "Authorization: token $GITHUB_TOKEN" \
              -H "Accept: application/vnd.github.v3+json" \
              "$GITHUB_API_BASE/user")
    
    # Проверяем, есть ли ошибка в ответе
    local error_msg=$(echo "$response" | jq -r '.message // empty')
    if [ -n "$error_msg" ] && [ "$error_msg" != "null" ]; then
        if [ "$error_msg" = "Bad credentials" ]; then
            log_error "Неверный GITHUB_TOKEN - токен истек или неверный"
            exit 1
        else
            log_error "Ошибка API: $error_msg"
            exit 1
        fi
    fi
    
    # Извлекаем имя пользователя - используем простой подход
    local username
    username=$(echo "$response" | tr '\n' ' ' | grep -o '"login":"[^"]*"' | cut -d'"' -f4)
    
    log_debug "Извлеченное имя пользователя: '$username'"
    
    if [ -n "$username" ]; then
        log_success "GitHub токен валиден для пользователя: $username"
        export GITHUB_USERNAME="$username"
    else
        log_error "Не удалось получить имя пользователя из токена"
        log_debug "Ответ API: $response"
        exit 1
    fi
}

# Валидация конфигурации
validate_config() {
    log_info "Валидация конфигурации..."
    
    # Проверяем репозиторий
    if [ "$TRIAD_REPO_OWNER" = "your-github-username" ]; then
        log_error "TRIAD_REPO_OWNER не настроен! Установите ваш GitHub username"
        exit 1
    fi
    
    # Проверяем размер машины
    local valid_machines=("basicLinux32gb" "basicLinux64gb" "basicLinux16gb" "machineLinux16gb" "machineLinux32gb" "machineLinux64gb")
    if [[ ! " ${valid_machines[*]} " =~ " ${CODESPACE_MACHINE} " ]]; then
        log_error "Неверный размер машины: $CODESPACE_MACHINE"
        log_info "Доступные размеры: ${valid_machines[*]}"
        exit 1
    fi
    
    # Проверяем локацию
    local valid_locations=("WestUs2" "EastUs" "WestEurope" "SoutheastAsia" "EastAsia" "AustraliaEast")
    if [[ ! " ${valid_locations[*]} " =~ " ${CODESPACE_LOCATION} " ]]; then
        log_error "Неверная локация: $CODESPACE_LOCATION"
        log_info "Доступные локации: ${valid_locations[*]}"
        exit 1
    fi
    
    # Проверяем таймаут
    if ! [[ "$CODESPACE_IDLE_TIMEOUT" =~ ^[0-9]+$ ]] || [ "$CODESPACE_IDLE_TIMEOUT" -lt 1 ] || [ "$CODESPACE_IDLE_TIMEOUT" -gt 480 ]; then
        log_error "Неверный таймаут бездействия: $CODESPACE_IDLE_TIMEOUT (должен быть 1-480 минут)"
        exit 1
    fi
    
    log_success "Конфигурация валидна"
}

# Функция для получения информации о репозитории
get_repo_info() {
    local owner="$1"
    local repo="$2"
    
    log_debug "Получаем информацию о репозитории $owner/$repo..."
    
    # Простой способ получения информации о репозитории
    local response
    response=$(curl -s -H "Authorization: token $GITHUB_TOKEN" \
              -H "Accept: application/vnd.github.v3+json" \
              "$GITHUB_API_BASE/repos/$owner/$repo")
    
    # Проверяем, есть ли ошибка в ответе
    if echo "$response" | jq -e '.message' > /dev/null 2>&1; then
        local error_msg=$(echo "$response" | jq -r '.message')
        log_error "Ошибка получения информации о репозитории: $error_msg"
        return 1
    fi
    
    echo "$response"
}

# Функция для создания Codespace
create_codespace() {
    local node_id="$1"
    local port="$2"
    local shard_id="$3"
    
    log_info "Создаем Codespace для ${node_id} (порт ${port}, shard ${shard_id})..."
    
    # Создаем JSON для создания Codespace
    local create_json=$(cat <<EOF
{
    "repository_id": $REPO_ID,
    "machine": "$CODESPACE_MACHINE",
    "location": "$CODESPACE_LOCATION",
    "idle_timeout_minutes": $CODESPACE_IDLE_TIMEOUT,
    "display_name": "$node_id",
    "retention_period_minutes": 480,
    "ref": "$TRIAD_BRANCH"
}
EOF
)
    
    log_debug "JSON для создания Codespace: $create_json"
    
    # Создаем Codespace
    # Используем временные файлы для корректной обработки HTTP кода и тела ответа
    local temp_response=$(mktemp)
    local temp_headers=$(mktemp)
    
    local http_code
    http_code=$(curl -s -w "%{http_code}" -o "$temp_response" -D "$temp_headers" -X POST \
        -H "Authorization: token $GITHUB_TOKEN" \
        -H "Accept: application/vnd.github.v3+json" \
        -H "Content-Type: application/json" \
        -d "$create_json" \
        "$GITHUB_API_BASE/user/codespaces")
    
    local body
    body=$(cat "$temp_response")
    
    # Очищаем временные файлы
    rm -f "$temp_response" "$temp_headers"
    
    if [ "$http_code" = "201" ]; then
        # Извлекаем ID Codespace
        local codespace_id=$(echo "$body" | jq -r '.id // empty')
        
        if [ -n "$codespace_id" ] && [ "$codespace_id" != "null" ]; then
            echo "$codespace_id" > "logs/${node_id}_codespace_id"
            log_success "Codespace создан: $codespace_id"
            return 0
        else
            log_error "Не удалось извлечь ID Codespace из ответа"
            log_debug "Ответ API: $body"
            return 1
        fi
    else
        log_error "Ошибка создания Codespace (HTTP $http_code): $body"
        return 1
    fi
}

# Функция для запуска команды в Codespace
run_in_codespace() {
    local codespace_id="$1"
    local command="$2"
    
    log_info "Выполняем команду в Codespace $codespace_id: $command"
    
    # Запускаем команду через GitHub CLI или API
    if command -v gh &> /dev/null; then
        log_debug "Используем GitHub CLI для выполнения команды"
        if gh codespace exec --codespace "$codespace_id" --command "$command"; then
            log_success "Команда выполнена успешно"
            return 0
        else
            log_error "Ошибка выполнения команды через GitHub CLI"
            return 1
        fi
    else
        log_warning "GitHub CLI не установлен, используем API..."
        # Альтернативная реализация через API
        run_command_via_api "$codespace_id" "$command"
    fi
}

# Альтернативная реализация через API (когда GitHub CLI недоступен)
run_command_via_api() {
    local codespace_id="$1"
    local command="$2"
    
    log_debug "Выполняем команду через API: $command"
    
    # Создаем временный файл с командой
    local temp_file="/tmp/triad_command_${codespace_id}_$(date +%s).sh"
    echo "#!/bin/bash" > "$temp_file"
    echo "cd /workspaces/TRIAD" >> "$temp_file"
    echo "$command" >> "$temp_file"
    chmod +x "$temp_file"
    
    # Отправляем файл в Codespace через API (если доступно)
    # Пока что возвращаем ошибку, так как API для выполнения команд ограничен
    log_error "API для выполнения команд недоступен. Установите GitHub CLI:"
    echo "  macOS: brew install gh"
    echo "  Ubuntu: sudo apt-get install gh"
    echo "  Или авторизуйтесь: gh auth login"
    
    rm -f "$temp_file"
    return 1
}

# Функция для остановки Codespace
stop_codespace() {
    local codespace_id="$1"
    
    log_info "Останавливаем Codespace $codespace_id..."
    
    # Используем временные файлы для корректной обработки HTTP кода и тела ответа
    local temp_response=$(mktemp)
    local temp_headers=$(mktemp)
    
    local http_code
    http_code=$(curl -s -w "%{http_code}" -o "$temp_response" -D "$temp_headers" -X POST \
        -H "Authorization: token $GITHUB_TOKEN" \
        -H "Accept: application/vnd.github.v3+json" \
        "$GITHUB_API_BASE/user/codespaces/$codespace_id/stop")
    
    local body
    body=$(cat "$temp_response")
    
    # Очищаем временные файлы
    rm -f "$temp_response" "$temp_headers"
    
    if [ "$http_code" = "200" ] || [ "$http_code" = "202" ]; then
        log_success "Codespace $codespace_id остановлен"
        return 0
    else
        log_error "Ошибка остановки Codespace (HTTP $http_code): $body"
        return 1
    fi
}

# Функция для удаления Codespace
delete_codespace() {
    local codespace_id="$1"
    
    log_info "Удаляем Codespace $codespace_id..."
    
    # Используем временные файлы для корректной обработки HTTP кода и тела ответа
    local temp_response=$(mktemp)
    local temp_headers=$(mktemp)
    
    local http_code
    http_code=$(curl -s -w "%{http_code}" -o "$temp_response" -D "$temp_headers" -X DELETE \
        -H "Authorization: token $GITHUB_TOKEN" \
        -H "Accept: application/vnd.github.v3+json" \
        "$GITHUB_API_BASE/user/codespaces/$codespace_id")
    
    local body
    body=$(cat "$temp_response")
    
    # Очищаем временные файлы
    rm -f "$temp_response" "$temp_headers"
    
    if [ "$http_code" = "200" ] || [ "$http_code" = "202" ]; then
        log_success "Codespace $codespace_id удален"
        return 0
    else
        log_error "Ошибка удаления Codespace (HTTP $http_code): $body"
        return 1
    fi
}

# Функция для получения статуса Codespace
get_codespace_status() {
    local codespace_id="$1"
    
    # Используем временные файлы для корректной обработки HTTP кода и тела ответа
    local temp_response=$(mktemp)
    local temp_headers=$(mktemp)
    
    local http_code
    http_code=$(curl -s -w "%{http_code}" -o "$temp_response" -D "$temp_headers" \
        -H "Authorization: token $GITHUB_TOKEN" \
        -H "Accept: application/vnd.github.v3+json" \
        "$GITHUB_API_BASE/user/codespaces/$codespace_id")
    
    local body
    body=$(cat "$temp_response")
    
    # Очищаем временные файлы
    rm -f "$temp_response" "$temp_headers"
    
    if [ "$http_code" = "200" ]; then
        local state=$(echo "$body" | jq -r '.state // "unknown"')
        if [ "$state" = "null" ]; then
            echo "unknown"
        else
            echo "$state"
        fi
    else
        log_error "Ошибка получения статуса Codespace (HTTP $http_code): $body"
        echo "error"
    fi
}

# Инициализация
init_github_api() {
    log_info "Инициализация GitHub API..."
    
    # Проверяем зависимости
    check_dependencies
    
    # Проверяем токен
    check_github_token
    
    # Валидируем конфигурацию
    validate_config
    
    # Получаем информацию о репозитории
    log_info "Получаем информацию о репозитории $TRIAD_REPO_OWNER/$TRIAD_REPO_NAME..."
    local repo_info
    if ! repo_info=$(get_repo_info "$TRIAD_REPO_OWNER" "$TRIAD_REPO_NAME"); then
        log_error "Не удалось получить информацию о репозитории"
        exit 1
    fi
    
    export REPO_ID=$(echo "$repo_info" | jq -r '.id // empty')
    
    if [ -z "$REPO_ID" ] || [ "$REPO_ID" = "null" ]; then
        log_error "Не удалось получить ID репозитория $TRIAD_REPO_OWNER/$TRIAD_REPO_NAME"
        log_info "Проверьте права доступа и существование репозитория"
        exit 1
    fi
    
    log_success "Репозиторий найден: ID $REPO_ID"
    
    # Создаем директорию для логов
    mkdir -p logs
    
    log_success "GitHub API готов к работе!"
}

# Экспортируем функции для использования в других скриптах
export -f setup_github_token
export -f check_github_token
export -f get_repo_info
export -f create_codespace
export -f run_in_codespace
export -f stop_codespace
export -f delete_codespace
export -f get_codespace_status
export -f init_github_api
