use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use log::{info, error, debug, warn};

/// GitHub Codespaces API интеграция
pub struct GitHubCodespaces {
    client: Client,
    token: String,
    owner: String,
    repo: String,
}

#[derive(Debug, Deserialize)]
pub struct Codespace {
    pub id: String,
    pub name: String,
    pub machine: String,
    pub state: String,
    pub url: String,
    pub web_url: String,
    pub location: String,
    pub idle_timeout_minutes: u32,
    pub retention_period_minutes: u32,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct CreateCodespaceRequest {
    pub repository_id: u64,
    pub machine: Option<String>,
    pub location: Option<String>,
    pub idle_timeout_minutes: Option<u32>,
    pub retention_period_minutes: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCodespaceResponse {
    pub codespace: Codespace,
}

impl GitHubCodespaces {
    pub fn new(token: String, owner: String, repo: String) -> Self {
        Self {
            client: Client::new(),
            token,
            owner,
            repo,
        }
    }

    /// Создает новый GitHub Codespace
    pub async fn create_codespace(&self, machine: &str, location: &str) -> Result<Codespace, String> {
        let url = "https://api.github.com/user/codespaces";
        
        let request = CreateCodespaceRequest {
            repository_id: 0, // Будет получен из repo
            machine: Some(machine.to_string()),
            location: Some(location.to_string()),
            idle_timeout_minutes: Some(480), // 8 часов
            retention_period_minutes: Some(1440), // 24 часа
        };

        let response = self.client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "TRIAD-Network")
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("Failed to create codespace: {}", e))?;

        if response.status().is_success() {
            let codespace_response: CreateCodespaceResponse = response.json().await
                .map_err(|e| format!("Failed to parse response: {}", e))?;
            
            info!("✅ Created GitHub Codespace: {} ({})", 
                  codespace_response.codespace.name, 
                  codespace_response.codespace.id);
            
            Ok(codespace_response.codespace)
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(format!("Failed to create codespace: {} - {}", response.status(), error_text))
        }
    }

    /// Получает список всех Codespaces
    pub async fn list_codespaces(&self) -> Result<Vec<Codespace>, String> {
        let url = "https://api.github.com/user/codespaces";
        
        let response = self.client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "TRIAD-Network")
            .send()
            .await
            .map_err(|e| format!("Failed to list codespaces: {}", e))?;

        if response.status().is_success() {
            let codespaces: Vec<Codespace> = response.json().await
                .map_err(|e| format!("Failed to parse response: {}", e))?;
            
            info!("📋 Found {} GitHub Codespaces", codespaces.len());
            Ok(codespaces)
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(format!("Failed to list codespaces: {} - {}", response.status(), error_text))
        }
    }

    /// Останавливает Codespace
    pub async fn stop_codespace(&self, codespace_id: &str) -> Result<(), String> {
        let url = format!("https://api.github.com/user/codespaces/{}/stop", codespace_id);
        
        let response = self.client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "TRIAD-Network")
            .send()
            .await
            .map_err(|e| format!("Failed to stop codespace: {}", e))?;

        if response.status().is_success() {
            info!("🛑 Stopped GitHub Codespace: {}", codespace_id);
            Ok(())
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(format!("Failed to stop codespace: {} - {}", response.status(), error_text))
        }
    }

    /// Удаляет Codespace
    pub async fn delete_codespace(&self, codespace_id: &str) -> Result<(), String> {
        let url = format!("https://api.github.com/user/codespaces/{}", codespace_id);
        
        let response = self.client
            .delete(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "TRIAD-Network")
            .send()
            .await
            .map_err(|e| format!("Failed to delete codespace: {}", e))?;

        if response.status().is_success() {
            info!("🗑️  Deleted GitHub Codespace: {}", codespace_id);
            Ok(())
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(format!("Failed to delete codespace: {} - {}", response.status(), error_text))
        }
    }

    /// Получает статус Codespace
    pub async fn get_codespace_status(&self, codespace_id: &str) -> Result<Codespace, String> {
        let url = format!("https://api.github.com/user/codespaces/{}", codespace_id);
        
        let response = self.client
            .get(url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "TRIAD-Network")
            .send()
            .await
            .map_err(|e| format!("Failed to get codespace status: {}", e))?;

        if response.status().is_success() {
            let codespace: Codespace = response.json().await
                .map_err(|e| format!("Failed to parse response: {}", e))?;
            
            Ok(codespace)
        } else {
            let error_text = response.text().await.unwrap_or_default();
            Err(format!("Failed to get codespace status: {} - {}", response.status(), error_text))
        }
    }

    /// Создает сеть из 10 Codespaces для TRIAD
    pub async fn create_triad_network(&self) -> Result<Vec<Codespace>, String> {
        info!("🚀 Creating TRIAD network with 10 GitHub Codespaces...");
        
        let mut codespaces = Vec::new();
        let machines = ["basicLinux32gb", "basicLinux32gb", "basicLinux32gb", 
                       "basicLinux32gb", "basicLinux32gb", "basicLinux32gb",
                       "basicLinux32gb", "basicLinux32gb", "basicLinux32gb", "basicLinux32gb"];
        let locations = ["WestUs2", "EastUs", "WestEurope", "SoutheastAsia", "AustraliaEast",
                        "BrazilSouth", "JapanEast", "SouthAfricaNorth", "CentralIndia", "NorthEurope"];

        for i in 0..10 {
            let machine = machines[i];
            let location = locations[i];
            
            info!("🌐 Creating Codespace {}/10: {} in {}", i + 1, machine, location);
            
            match self.create_codespace(machine, location).await {
                Ok(codespace) => {
                    codespaces.push(codespace);
                    info!("✅ Codespace {}/10 created: {}", i + 1, codespace.name);
                },
                Err(e) => {
                    error!("❌ Failed to create Codespace {}/10: {}", i + 1, e);
                    // Продолжаем создание остальных
                }
            }
            
            // Небольшая задержка между запросами
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }

        info!("🎉 Created {} GitHub Codespaces for TRIAD network", codespaces.len());
        Ok(codespaces)
    }

    /// Останавливает всю сеть TRIAD
    pub async fn stop_triad_network(&self) -> Result<(), String> {
        info!("🛑 Stopping TRIAD network...");
        
        let codespaces = self.list_codespaces().await?;
        let triad_codespaces: Vec<_> = codespaces
            .into_iter()
            .filter(|c| c.name.starts_with("triad-node-"))
            .collect();

        info!("🛑 Found {} TRIAD codespaces to stop", triad_codespaces.len());

        for codespace in triad_codespaces {
            if let Err(e) = self.stop_codespace(&codespace.id).await {
                warn!("⚠️  Failed to stop codespace {}: {}", codespace.name, e);
            }
        }

        info!("✅ TRIAD network stopped");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_github_codespaces_new() {
        let github = GitHubCodespaces::new(
            "test_token".to_string(),
            "test_owner".to_string(),
            "test_repo".to_string(),
        );
        
        assert_eq!(github.token, "test_token");
        assert_eq!(github.owner, "test_owner");
        assert_eq!(github.repo, "test_repo");
    }
}
