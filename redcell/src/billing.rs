use crate::config::StripeConfig;
use anyhow::Context;
use stripe;

pub struct BillingClient {
    pub client: stripe::Client,
    config: StripeConfig,
}

impl BillingClient {
    pub fn new(config: StripeConfig) -> Self {
        Self {
            client: stripe::Client::new(config.secret_key.clone()),
            config,
        }
    }

    pub async fn create_customer(&self, email: &str) -> anyhow::Result<String> {
        let customer = stripe::Customer::create(
            &self.client,
            stripe::CreateCustomer {
                email: Some(email),
                ..Default::default()
            },
        )
        .await?;
        Ok(customer.id.to_string())
    }

    pub async fn create_checkout_session(&self, customer_id: &str) -> anyhow::Result<String> {
        let params = stripe::CreateCheckoutSession {
            customer: Some(customer_id.parse()?),
            mode: Some(stripe::CheckoutSessionMode::Subscription),
            line_items: Some(vec![stripe::CreateCheckoutSessionLineItems {
                price: Some(self.config.price_id.clone()),
                quantity: Some(1),
                ..Default::default()
            }]),
            success_url: Some(&self.config.success_url),
            cancel_url: Some(&self.config.cancel_url),
            ..Default::default()
        };

        let session = stripe::CheckoutSession::create(&self.client, params).await?;
        session.url.context("checkout session missing url")
    }
}
