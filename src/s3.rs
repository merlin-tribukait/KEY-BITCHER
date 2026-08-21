use crate::config::S3Config;
use anyhow::Result;
use aws_config::{self, BehaviorVersion};
use aws_sdk_s3 as s3;
use aws_sdk_s3::types::BucketLocationConstraint;
use aws_smithy_http_client::{tls::rustls_provider::CryptoMode, tls::Provider, Builder};

pub fn endpoint_for(cfg: &S3Config) -> String {
    format!("https://s3.{}.backblazeb2.com", cfg.region)
}

async fn build_client(cfg: &S3Config) -> s3::Client {
    let http_client = Builder::new()
        .tls_provider(Provider::rustls(CryptoMode::Ring))
        .build_https();

    let shared_config = aws_config::defaults(BehaviorVersion::latest())
        .region(aws_config::Region::new(cfg.region.clone()))
        .http_client(http_client.clone())
        .load()
        .await;

    let s3_config = s3::config::Builder::from(&shared_config)
        .endpoint_url(endpoint_for(cfg))
        .http_client(http_client)
        .build();

    s3::Client::from_conf(s3_config)
}

pub async fn fetch_object_to_string(cfg: &S3Config) -> Result<String> {
    let client = build_client(cfg).await;

    let resp = client
        .get_object()
        .bucket(&cfg.bucket)
        .key(&cfg.object_key)
        .send()
        .await?;

    let data = resp.body.collect().await?;
    let bytes = data.into_bytes();
    let s = String::from_utf8(bytes.to_vec())?;
    Ok(s)
}

pub async fn put_object_string(cfg: &S3Config, body: &str) -> Result<()> {
    let client = build_client(cfg).await;

    client
        .put_object()
        .bucket(&cfg.bucket)
        .key(&cfg.object_key)
        .body(aws_sdk_s3::primitives::ByteStream::from(
            body.as_bytes().to_vec(),
        ))
        .content_type("application/json")
        .send()
        .await?;

    Ok(())
}

pub async fn bucket_exists(cfg: &S3Config) -> Result<bool> {
    let client = build_client(cfg).await;
    let out = client.head_bucket().bucket(&cfg.bucket).send().await;
    match out {
        Ok(_) => Ok(true),
        Err(e)
            if e.raw_response()
                .map(|r| r.status().as_u16() == 404)
                .unwrap_or(false) =>
        {
            Ok(false)
        }
        Err(e) => Err(e.into()),
    }
}

pub async fn create_bucket(cfg: &S3Config) -> Result<()> {
    let client = build_client(cfg).await;

    match client.create_bucket().bucket(&cfg.bucket).send().await {
        Ok(_) => Ok(()),
        Err(first_err) => {
            // Some endpoints (Backblaze) accept the bucket without a location constraint.
            let constraint = BucketLocationConstraint::try_parse(&cfg.region)
                .unwrap_or(BucketLocationConstraint::UsWest2);
            match client
                .create_bucket()
                .bucket(&cfg.bucket)
                .create_bucket_configuration(
                    aws_sdk_s3::types::CreateBucketConfiguration::builder()
                        .location_constraint(constraint)
                        .build(),
                )
                .send()
                .await
            {
                Ok(_) => Ok(()),
                Err(_) => Err(first_err.into()),
            }
        }
    }
}

pub async fn list_buckets(cfg: &S3Config) -> Result<Vec<String>> {
    let client = build_client(cfg).await;
    let out = client.list_buckets().send().await?;
    let mut names = Vec::new();
    if let Some(buckets) = out.buckets {
        for b in buckets {
            if let Some(name) = b.name {
                names.push(name);
            }
        }
    }
    Ok(names)
}

/// Returns "Enabled", "Suspended" or "Never"/"Unversioned" for the bucket.
pub async fn versioning_status(cfg: &S3Config) -> Result<String> {
    let client = build_client(cfg).await;
    let out = client
        .get_bucket_versioning()
        .bucket(&cfg.bucket)
        .send()
        .await?;
    let s = out.status().map(|st| st.as_str());
    Ok(match s {
        Some("Enabled") => "Enabled".to_string(),
        Some("Suspended") => "Suspended".to_string(),
        _ => "Never enabled / Unversioned".to_string(),
    })
}

/// Enables S3 object versioning on the bucket (idempotent).
pub async fn enable_versioning(cfg: &S3Config) -> Result<()> {
    let client = build_client(cfg).await;
    client
        .put_bucket_versioning()
        .bucket(&cfg.bucket)
        .versioning_configuration(
            aws_sdk_s3::types::VersioningConfiguration::builder()
                .status(aws_sdk_s3::types::BucketVersioningStatus::Enabled)
                .build(),
        )
        .send()
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> S3Config {
        S3Config {
            bucket: "wave-secrets-bucket".into(),
            region: "eu-central-003".into(),
            object_key: "secrets/ai-keys.json".into(),
        }
    }

    #[test]
    fn endpoint_uses_region() {
        assert_eq!(
            endpoint_for(&cfg()),
            "https://s3.eu-central-003.backblazeb2.com"
        );
    }
}
