#!/bin/bash

# 📊 TRIAD Distributed Network Status Checker
# Проверяет статус всех узлов в распределенной сети через GitHub Codespaces

set -e

echo "📊 Проверка статуса TRIAD Distributed Network..."
echo "================================================"

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

# Статистика
total_nodes=0
active_nodes=0
inactive_nodes=0
failed_nodes=0

# Функция для проверки статуса узла
check_node_status() {
    local node_id="$1"
    local port="$2"
    local shard_id="$3"
    local location="$4"
    
    echo -e "${BLUE}🔍 Проверка ${node_id}...${NC}"
    echo "  📍 Порт: ${port}"
    echo "  🔢 Shard: ${shard_id}"
    echo "  🌍 Локация: ${location}"
    
    local active_file="logs/${node_id}_active"
    if [ -f "$active_file" ]; then
        IFS=':' read -r codespace_id node_port node_shard <<< "$(cat "$active_file")"
        
        if [ "$node_port" = "$port" ] && [ "$node_shard" = "$shard_id" ]; then
            # Проверяем статус Codespace
            local codespace_status=$(get_codespace_status "$codespace_id")
            echo -e "  📊 Codespace статус: ${codespace_status}"
            
            if [ "$codespace_status" = "Available" ]; then
                echo -e "  ✅ ${GREEN}ACTIVE${NC} (Codespace: ${codespace_id})"
                
                # Проверяем, работает ли узел
                if check_node_health "$codespace_id" "$node_id" "$port"; then
                    echo -e "  🟢 Узел отвечает на health check"
                    active_nodes=$((active_nodes + 1))
                else
                    echo -e "  🟡 Узел не отвечает на health check"
                    inactive_nodes=$((inactive_nodes + 1))
                fi
                
            elif [ "$codespace_status" = "Failed" ] || [ "$codespace_status" = "Canceled" ]; then
                echo -e "  ❌ ${RED}FAILED${NC} (Codespace: ${codespace_id})"
                failed_nodes=$((failed_nodes + 1))
            else
                echo -e "  ⚠️  ${YELLOW}${codespace_status}${NC} (Codespace: ${codespace_id})"
                inactive_nodes=$((inactive_nodes + 1))
            fi
            
        else
            echo -e "  ❌ ${RED}CONFIG MISMATCH${NC}"
            failed_nodes=$((failed_nodes + 1))
        fi
    else
        echo -e "  ❌ ${RED}NOT ACTIVE${NC}"
        inactive_nodes=$((inactive_nodes + 1))
    fi
    
    echo ""
}

# Функция для проверки здоровья узла
check_node_health() {
    local codespace_id="$1"
    local node_id="$2"
    local port="$3"
    
    # Пытаемся выполнить health check через Codespace
    local health_cmd="curl -s http://localhost:${port}/health 2>/dev/null || echo 'unavailable'"
    local health_result=$(run_in_codespace "$codespace_id" "$health_cmd")
    
    if [ "$health_result" != "unavailable" ] && [ -n "$health_result" ]; then
        return 0  # Узел отвечает
    else
        return 1  # Узел не отвечает
    fi
}

# Функция для получения детальной информации об узле
get_node_details() {
    local node_id="$1"
    local active_file="logs/${node_id}_active"
    
    if [ -f "$active_file" ]; then
        IFS=':' read -r codespace_id node_port node_shard <<< "$(cat "$active_file")"
        
        echo -e "${PURPLE}📋 Детальная информация для ${node_id}:${NC}"
        echo "  🆔 Codespace ID: ${codespace_id}"
        echo "  📍 Порт: ${node_port}"
        echo "  🔢 Shard: ${node_shard}"
        
        # Получаем статус Codespace
        local status=$(get_codespace_status "$codespace_id")
        echo "  📊 Статус: ${status}"
        
        # Получаем информацию о логах
        local log_cmd="wc -l < logs/node_${node_id}.log 2>/dev/null || echo '0'"
        local log_lines=$(run_in_codespace "$codespace_id" "$log_cmd")
        echo "  📄 Строк в логах: ${log_lines}"
        
        # Получаем информацию о процессе
        local process_cmd="ps aux | grep 'quantum_consensus_demo.*${node_id}' | grep -v grep | wc -l"
        local process_count=$(run_in_codespace "$codespace_id" "$process_cmd")
        echo "  🔄 Процессов узла: ${process_count}"
        
        echo ""
    fi
}

# Функция для проверки сетевых соединений
check_network_connections() {
    echo -e "${BLUE}🔗 Проверка сетевых соединений...${NC}"
    
    local connected_pairs=0
    local total_pairs=0
    
    # Проверяем связи между активными узлами
    for node_id in "${!NODES[@]}"; do
        local active_file="logs/${node_id}_active"
        if [ -f "$active_file" ]; then
            IFS=':' read -r codespace_id node_port node_shard <<< "$(cat "$active_file")"
            
            for other_node_id in "${!NODES[@]}"; do
                if [ "$node_id" != "$other_node_id" ]; then
                    local other_active_file="logs/${other_node_id}_active"
                    if [ -f "$other_active_file" ]; then
                        IFS=':' read -r other_codespace_id other_node_port other_node_shard <<< "$(cat "$other_active_file")"
                        
                        total_pairs=$((total_pairs + 1))
                        
                        # Проверяем, может ли узел видеть другой узел
                        if check_peer_connection "$codespace_id" "$node_id" "$other_node_id"; then
                            echo -e "  ✅ ${node_id} ↔ ${other_node_id}: соединение установлено"
                            connected_pairs=$((connected_pairs + 1))
                        else
                            echo -e "  ❌ ${node_id} ↔ ${other_node_id}: соединение не установлено"
                        fi
                    fi
                fi
            done
        fi
    done
    
    echo ""
    echo -e "${BLUE}📊 Статистика соединений:${NC}"
    echo -e "  Всего пар: ${total_pairs}"
    echo -e "  Соединено: ${GREEN}${connected_pairs}${NC}"
    echo -e "  Не соединено: ${RED}$((total_pairs - connected_pairs))${NC}"
    
    if [ $total_pairs -gt 0 ]; then
        local connection_rate=$((connected_pairs * 100 / total_pairs))
        echo -e "  Процент соединений: ${connection_rate}%"
    fi
}

# Функция для проверки соединения между пирами
check_peer_connection() {
    local codespace_id="$1"
    local node_id="$2"
    local peer_id="$3"
    
    # Команда для проверки пиров
    local peer_cmd="curl -s http://localhost:8080/peers 2>/dev/null | grep -q '${peer_id}' && echo 'connected' || echo 'disconnected'"
    local result=$(run_in_codespace "$codespace_id" "$peer_cmd")
    
    [ "$result" = "connected" ]
}

# Функция для получения метрик производительности
get_performance_metrics() {
    echo -e "${BLUE}📈 Метрики производительности...${NC}"
    
    local total_tps=0
    local total_latency=0
    local total_consensus_success=0
    local metrics_count=0
    
    for node_id in "${!NODES[@]}"; do
        local active_file="logs/${node_id}_active"
        if [ -f "$active_file" ]; then
            IFS=':' read -r codespace_id node_port node_shard <<< "$(cat "$active_file")"
            
            # Получаем метрики узла
            local metrics_cmd="curl -s http://localhost:${node_port}/metrics 2>/dev/null || echo '{}'"
            local metrics=$(run_in_codespace "$codespace_id" "$metrics_cmd")
            
            if [ "$metrics" != "{}" ] && [ -n "$metrics" ]; then
                echo -e "  📊 ${node_id}:"
                
                # Извлекаем TPS
                local tps=$(echo "$metrics" | jq -r '.tps // 0')
                echo -e "    🚀 TPS: ${tps}"
                total_tps=$((total_tps + tps))
                
                # Извлекаем латентность
                local latency=$(echo "$metrics" | jq -r '.latency_ms // 0')
                echo -e "    ⏱️  Латентность: ${latency}ms"
                total_latency=$((total_latency + latency))
                
                # Извлекаем успешность консенсуса
                local consensus_success=$(echo "$metrics" | jq -r '.consensus_success_rate // 0')
                echo -e "    ✅ Консенсус: ${consensus_success}%"
                total_consensus_success=$((total_consensus_success + consensus_success))
                
                metrics_count=$((metrics_count + 1))
            fi
        fi
    done
    
    if [ $metrics_count -gt 0 ]; then
        echo ""
        echo -e "${BLUE}📊 Общие метрики сети:${NC}"
        echo -e "  🚀 Общий TPS: ${total_tps}"
        echo -e "  ⏱️  Средняя латентность: $((total_latency / metrics_count))ms"
        echo -e "  ✅ Средняя успешность консенсуса: $((total_consensus_success / metrics_count))%"
    fi
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
        total_nodes=$((total_nodes + 1))
    done
    echo ""
    
    echo -e "${BLUE}🔍 Проверка статуса узлов...${NC}"
    for node_id in "${!NODES[@]}"; do
        IFS=':' read -r port shard_id location <<< "${NODES[$node_id]}"
        check_node_status "$node_id" "$port" "$shard_id" "$location"
    done
    
    echo ""
    echo -e "${BLUE}📊 Общая статистика:${NC}"
    echo -e "  Всего узлов: ${total_nodes}"
    echo -e "  Активных: ${GREEN}${active_nodes}${NC}"
    echo -e "  Неактивных: ${YELLOW}${inactive_nodes}${NC}"
    echo -e "  Ошибок: ${RED}${failed_nodes}${NC}"
    
    if [ $total_nodes -gt 0 ]; then
        local availability=$((active_nodes * 100 / total_nodes))
        echo -e "  Доступность: ${availability}%"
    fi
    
    echo ""
    
    # Получаем детальную информацию для активных узлов
    if [ $active_nodes -gt 0 ]; then
        echo -e "${BLUE}📋 Детальная информация об активных узлах:${NC}"
        for node_id in "${!NODES[@]}"; do
            local active_file="logs/${node_id}_active"
            if [ -f "$active_file" ]; then
                get_node_details "$node_id"
            fi
        done
    fi
    
    # Проверяем сетевые соединения
    if [ $active_nodes -gt 1 ]; then
        check_network_connections
    fi
    
    # Получаем метрики производительности
    if [ $active_nodes -gt 0 ]; then
        get_performance_metrics
    fi
    
    echo ""
    if [ $failed_nodes -eq 0 ] && [ $inactive_nodes -eq 0 ]; then
        echo -e "${GREEN}🎉 Все узлы работают! Сеть полностью функциональна.${NC}"
    elif [ $active_nodes -gt 0 ]; then
        echo -e "${YELLOW}⚠️  ${inactive_nodes} узлов неактивны, ${failed_nodes} с ошибками.${NC}"
    else
        echo -e "${RED}❌ Все узлы не работают! Проверьте логи.${NC}"
    fi
    
    echo "================================================"
}

# Запуск основной логики
main "$@"
