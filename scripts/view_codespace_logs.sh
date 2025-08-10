#!/bin/bash

# 📄 TRIAD Codespace Logs Viewer
# Просматривает логи узлов в GitHub Codespaces

set -e

echo "📄 TRIAD Codespace Logs Viewer..."
echo "=================================="

# Цвета для вывода
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
PURPLE='\033[0;35m'
CYAN='\033[0;36m'
NC='\033[0m' # No Color

# Загружаем конфигурацию GitHub API
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "$SCRIPT_DIR/github_config.sh"

# Конфигурация узлов сети
declare -A NODES
NODES["triad-node-0"]="8080:0:WestUs2"
NODES["triad-node-1"]="8081:1:EastUs"
NODES["triad-node-2"]="8082:2:WestEurope"
NODES["triad-node-3"]="8083:3:SoutheastAsia"
NODES["triad-node-4"]="8084:4:WestUs2"
NODES["triad-node-5"]="8085:0:EastUs"
NODES["triad-node-6"]="8086:1:WestEurope"
NODES["triad-node-7"]="8087:2:SoutheastAsia"
NODES["triad-node-8"]="8088:3:WestUs2"
NODES["triad-node-9"]="8089:4:EastUs"

# Функция для просмотра логов узла
view_node_logs() {
    local node_id="$1"
    local port="$2"
    local shard_id="$3"
    local lines="$4"
    
    echo -e "${BLUE}📄 Логи ${node_id} (порт ${port}, shard ${shard_id}):${NC}"
    echo "=================================================="
    
    local active_file="logs/${node_id}_active"
    if [ -f "$active_file" ]; then
        IFS=':' read -r codespace_id node_port node_shard <<< "$(cat "$active_file")"
        
        if [ "$node_port" = "$port" ] && [ "$node_shard" = "$shard_id" ]; then
            echo -e "  🆔 Codespace ID: ${codespace_id}"
            
            # Получаем логи узла
            local log_cmd="tail -n ${lines} logs/node_${node_id}.log 2>/dev/null || echo 'Логи не найдены'"
            local logs=$(run_in_codespace "$codespace_id" "$log_cmd")
            
            if [ "$logs" != "Логи не найдены" ]; then
                echo -e "${CYAN}📋 Последние ${lines} строк логов:${NC}"
                echo ""
                echo "$logs"
            else
                echo -e "${YELLOW}⚠️  Логи не найдены${NC}"
            fi
            
            # Получаем размер файла логов
            local size_cmd="wc -l < logs/node_${node_id}.log 2>/dev/null || echo '0'"
            local log_lines=$(run_in_codespace "$codespace_id" "$size_cmd")
            echo ""
            echo -e "${PURPLE}📊 Всего строк в логах: ${log_lines}${NC}"
            
        else
            echo -e "${RED}❌ Несоответствие конфигурации${NC}"
        fi
    else
        echo -e "${RED}❌ Узел не активен${NC}"
    fi
    
    echo ""
}

# Функция для просмотра всех логов
view_all_logs() {
    local lines="$1"
    
    echo -e "${BLUE}📄 Просмотр всех логов (последние ${lines} строк):${NC}"
    echo ""
    
    for node_id in "${!NODES[@]}"; do
        IFS=':' read -r port shard_id location <<< "${NODES[$node_id]}"
        view_node_logs "$node_id" "$port" "$shard_id" "$lines"
    done
}

# Функция для поиска в логах
search_logs() {
    local search_term="$1"
    local lines="$2"
    
    echo -e "${BLUE}🔍 Поиск '${search_term}' в логах (последние ${lines} строк):${NC}"
    echo ""
    
    local found_in_nodes=0
    
    for node_id in "${!NODES[@]}"; do
        IFS=':' read -r port shard_id location <<< "${NODES[$node_id]}"
        
        local active_file="logs/${node_id}_active"
        if [ -f "$active_file" ]; then
            IFS=':' read -r codespace_id node_port node_shard <<< "$(cat "$active_file")"
            
            # Ищем термин в логах
            local search_cmd="grep -n '${search_term}' logs/node_${node_id}.log | tail -n ${lines} 2>/dev/null || echo 'не найдено'"
            local search_results=$(run_in_codespace "$codespace_id" "$search_cmd")
            
            if [ "$search_results" != "не найдено" ] && [ -n "$search_results" ]; then
                echo -e "${GREEN}✅ Найдено в ${node_id}:${NC}"
                echo "$search_results"
                echo ""
                found_in_nodes=$((found_in_nodes + 1))
            fi
        fi
    done
    
    if [ $found_in_nodes -eq 0 ]; then
        echo -e "${YELLOW}⚠️  Термин '${search_term}' не найден ни в одном узле${NC}"
    else
        echo -e "${GREEN}📊 Найдено в ${found_in_nodes} узлах${NC}"
    fi
}

# Функция для просмотра ошибок
view_errors() {
    local lines="$1"
    
    echo -e "${BLUE}❌ Просмотр ошибок в логах (последние ${lines} строк):${NC}"
    echo ""
    
    local error_nodes=0
    
    for node_id in "${!NODES[@]}"; do
        IFS=':' read -r port shard_id location <<< "${NODES[$node_id]}"
        
        local active_file="logs/${node_id}_active"
        if [ -f "$active_file" ]; then
            IFS=':' read -r codespace_id node_port node_shard <<< "$(cat "$active_file")"
            
            # Ищем ошибки в логах
            local error_cmd="grep -i -E '(error|fail|exception|panic|warning)' logs/node_${node_id}.log | tail -n ${lines} 2>/dev/null || echo 'ошибок нет'"
            local error_results=$(run_in_codespace "$codespace_id" "$error_cmd")
            
            if [ "$error_results" != "ошибок нет" ] && [ -n "$error_results" ]; then
                echo -e "${RED}❌ Ошибки в ${node_id}:${NC}"
                echo "$error_results"
                echo ""
                error_nodes=$((error_nodes + 1))
            fi
        fi
    done
    
    if [ $error_nodes -eq 0 ]; then
        echo -e "${GREEN}✅ Ошибок не найдено${NC}"
    else
        echo -e "${RED}📊 Ошибки найдены в ${error_nodes} узлах${NC}"
    fi
}

# Функция для просмотра метрик
view_metrics() {
    echo -e "${BLUE}📈 Просмотр метрик узлов:${NC}"
    echo ""
    
    local metrics_nodes=0
    
    for node_id in "${!NODES[@]}"; do
        IFS=':' read -r port shard_id location <<< "${NODES[$node_id]}"
        
        local active_file="logs/${node_id}_active"
        if [ -f "$active_file" ]; then
            IFS=':' read -r codespace_id node_port node_shard <<< "$(cat "$active_file")"
            
            # Получаем метрики узла
            local metrics_cmd="curl -s http://localhost:${port}/metrics 2>/dev/null || echo '{}'"
            local metrics=$(run_in_codespace "$codespace_id" "$metrics_cmd")
            
            if [ "$metrics" != "{}" ] && [ -n "$metrics" ]; then
                echo -e "${GREEN}📊 ${node_id}:${NC}"
                
                # Извлекаем и отображаем метрики
                local tps=$(echo "$metrics" | jq -r '.tps // "N/A"')
                local latency=$(echo "$metrics" | jq -r '.latency_ms // "N/A"')
                local consensus_success=$(echo "$metrics" | jq -r '.consensus_success_rate // "N/A"')
                local active_peers=$(echo "$metrics" | jq -r '.active_peers // "N/A"')
                
                echo -e "  🚀 TPS: ${tps}"
                echo -e "  ⏱️  Латентность: ${latency}ms"
                echo -e "  ✅ Консенсус: ${consensus_success}%"
                echo -e "  📡 Активные пиры: ${active_peers}"
                echo ""
                
                metrics_nodes=$((metrics_nodes + 1))
            fi
        fi
    done
    
    if [ $metrics_nodes -eq 0 ]; then
        echo -e "${YELLOW}⚠️  Метрики недоступны${NC}"
    else
        echo -e "${GREEN}📊 Метрики получены из ${metrics_nodes} узлов${NC}"
    fi
}

# Функция для интерактивного просмотра
interactive_view() {
    echo -e "${BLUE}🎮 Интерактивный просмотр логов:${NC}"
    echo ""
    
    while true; do
        echo -e "${CYAN}Выберите действие:${NC}"
        echo "  1. Просмотр всех логов"
        echo "  2. Просмотр логов конкретного узла"
        echo "  3. Поиск в логах"
        echo "  4. Просмотр ошибок"
        echo "  5. Просмотр метрик"
        echo "  6. Выход"
        echo ""
        
        read -p "Введите номер (1-6): " choice
        
        case $choice in
            1)
                read -p "Количество последних строк (по умолчанию 50): " lines
                lines=${lines:-50}
                view_all_logs "$lines"
                ;;
            2)
                echo -e "${CYAN}Доступные узлы:${NC}"
                local i=1
                for node_id in "${!NODES[@]}"; do
                    echo "  ${i}. ${node_id}"
                    i=$((i + 1))
                done
                echo ""
                
                read -p "Выберите номер узла (1-10): " node_choice
                if [[ "$node_choice" =~ ^[1-9]$|^10$ ]]; then
                    local i=1
                    for node_id in "${!NODES[@]}"; do
                        if [ $i -eq $node_choice ]; then
                            IFS=':' read -r port shard_id location <<< "${NODES[$node_id]}"
                            read -p "Количество последних строк (по умолчанию 50): " lines
                            lines=${lines:-50}
                            view_node_logs "$node_id" "$port" "$shard_id" "$lines"
                            break
                        fi
                        i=$((i + 1))
                    done
                else
                    echo -e "${RED}❌ Неверный номер узла${NC}"
                fi
                ;;
            3)
                read -p "Введите поисковый термин: " search_term
                if [ -n "$search_term" ]; then
                    read -p "Количество последних строк (по умолчанию 20): " lines
                    lines=${lines:-20}
                    search_logs "$search_term" "$lines"
                else
                    echo -e "${RED}❌ Поисковый термин не может быть пустым${NC}"
                fi
                ;;
            4)
                read -p "Количество последних строк (по умолчанию 20): " lines
                lines=${lines:-20}
                view_errors "$lines"
                ;;
            5)
                view_metrics
                ;;
            6)
                echo -e "${GREEN}👋 До свидания!${NC}"
                exit 0
                ;;
            *)
                echo -e "${RED}❌ Неверный выбор. Попробуйте снова.${NC}"
                ;;
        esac
        
        echo ""
        read -p "Нажмите Enter для продолжения..."
        echo ""
    done
}

# Основная логика
main() {
    echo -e "${BLUE}🔧 Инициализация GitHub API...${NC}"
    init_github_api
    
    echo ""
    echo -e "${BLUE}📋 Конфигурация сети:${NC}"
    for node_id in "${!NODES[@]}"; do
        IFS=':' read -r port shard_id location <<< "${NODES[$node_id]}"
        echo -e "  ${node_id}: порт ${port}, shard ${shard_id}, локация ${location}"
    done
    echo ""
    
    # Проверяем аргументы командной строки
    if [ $# -eq 0 ]; then
        # Интерактивный режим
        interactive_view
    elif [ "$1" = "all" ]; then
        # Просмотр всех логов
        local lines=${2:-50}
        view_all_logs "$lines"
    elif [ "$1" = "node" ] && [ -n "$2" ]; then
        # Просмотр логов конкретного узла
        local node_id="$2"
        local lines=${3:-50}
        
        if [[ "${NODES[$node_id]}" ]]; then
            IFS=':' read -r port shard_id location <<< "${NODES[$node_id]}"
            view_node_logs "$node_id" "$port" "$shard_id" "$lines"
        else
            echo -e "${RED}❌ Узел '${node_id}' не найден${NC}"
            echo -e "${CYAN}Доступные узлы:${NC}"
            for node_id in "${!NODES[@]}"; do
                echo "  ${node_id}"
            done
        fi
    elif [ "$1" = "search" ] && [ -n "$2" ]; then
        # Поиск в логах
        local search_term="$2"
        local lines=${3:-20}
        search_logs "$search_term" "$lines"
    elif [ "$1" = "errors" ]; then
        # Просмотр ошибок
        local lines=${2:-20}
        view_errors "$lines"
    elif [ "$1" = "metrics" ]; then
        # Просмотр метрик
        view_metrics
    else
        echo -e "${YELLOW}💡 Использование:${NC}"
        echo "  $0                    # Интерактивный режим"
        echo "  $0 all [строки]      # Просмотр всех логов"
        echo "  $0 node <узел> [строки] # Просмотр логов узла"
        echo "  $0 search <термин> [строки] # Поиск в логах"
        echo "  $0 errors [строки]   # Просмотр ошибок"
        echo "  $0 metrics           # Просмотр метрик"
        echo ""
        echo -e "${CYAN}Примеры:${NC}"
        echo "  $0 all 100           # Последние 100 строк всех логов"
        echo "  $0 node triad-node-0 50  # 50 строк логов узла triad-node-0"
        echo "  $0 search 'consensus' 30 # Поиск 'consensus' в последних 30 строках"
    fi
}

# Запуск основной логики
main "$@"
