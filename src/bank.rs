use std::collections::HashMap;

use crate::{
    account::Account,
    amount::Amount,
    transaction::{Kind, Transaction},
};

pub type AccountID = u16;
pub type TransactionID = u32;

/// A basic bank. Will store all accounts in memory, transactions of certain types that might be
/// needed in the future, and any ongoing disputes that might need to be resolved. All logic for
/// how the bank operates is applied at this level, the underlying account has no knowledge of how
/// transactions get processed.
pub struct Bank {
    // Current state of all accounts
    accounts: HashMap<AccountID, Account>,

    // History of all transactions which are `Kind::Deposit` which might be eventually disputed.
    transactions: HashMap<TransactionID, Transaction>,

    // Current ongoing disputes.
    disputes: HashMap<TransactionID, Transaction>,
}

impl Bank {
    pub fn new() -> Self {
        Self {
            accounts: HashMap::new(),
            transactions: HashMap::new(),
            disputes: HashMap::new(),
        }
    }

    // Public exposure. Ensure to report valid floating point values.
    pub fn accounts_iter(&self) -> impl Iterator<Item = (u16, Amount, Amount, Amount, bool)> + '_ {
        self.accounts.iter().map(|(id, account)| {
            (
                *id,
                account.available(),
                account.held(),
                account.total(),
                account.is_locked(),
            )
        })
    }

    pub fn process_transaction(&mut self, transaction: Transaction) {
        match transaction.kind() {
            Kind::Deposit => self.process_deposit(transaction),
            Kind::Withdrawal => self.process_withdrawl(transaction),
            Kind::Dispute => self.process_dispute(transaction),
            Kind::Resolve => self.process_resolve(transaction),
            Kind::Chargeback => self.process_chargeback(transaction),
        };
    }

    // TODO: Handle credit missing amounts
    // TODO: Handle duplicate transactions id's
    fn process_deposit(&mut self, transaction: Transaction) {
        if let Some(amount) = transaction.amount() {
            let account = self
                .accounts
                .entry(transaction.client())
                .or_insert_with(Account::new);

            account.credit(amount);
            self.transactions.insert(transaction.id(), transaction);
        }
    }

    // TODO: Handle accounts missing funds for debit
    // TODO: Handle transaction missing amounts
    // TODO: Handle debit from non-existant accounts
    fn process_withdrawl(&mut self, transaction: Transaction) {
        if let Some(account) = self.accounts.get_mut(&transaction.client()) {
            if let Some(amount) = transaction.amount() {
                let _ = account.try_debit(amount);
            }
        }
    }

    // TODO: Handle multiple disputes at once for the same transaction id
    // TODO: Handle disputes where values have already been withdrawn or are not available
    // TODO: Handle disputes where the transaction in dispute is not from the same client
    fn process_dispute(&mut self, transaction: Transaction) {
        if let Some(old_transaction) = self.transactions.get(&transaction.id()) {
            if let Some(account) = self.accounts.get_mut(&old_transaction.client()) {
                // unwrap is safe because we only would have inserted into `self.transactions` if
                // there was a valid amount.
                if let Ok(_) = account.try_dispute(old_transaction.amount().unwrap()) {
                    self.disputes
                        .insert(transaction.id(), old_transaction.clone());
                }
            }
        }
    }

    fn process_resolve(&mut self, transaction: Transaction) {
        if let Some(transaction) = self.disputes.remove(&transaction.id()) {
            if let Some(account) = self.accounts.get_mut(&transaction.client()) {
                // unwrap is safe because we only would have inserted into `self.transactions` if
                // there was a valid amount.
                let _ = account.try_resolve(transaction.amount().unwrap());
            }
        }
    }

    fn process_chargeback(&mut self, transaction: Transaction) {
        if let Some(transaction) = self.disputes.remove(&transaction.id()) {
            if let Some(account) = self.accounts.get_mut(&transaction.client()) {
                // unwrap is safe because we only would have inserted into `self.transactions` if
                // there was a valid amount.
                let _ = account.try_chargeback(transaction.amount().unwrap());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iterator() {
        let mut account = Account::new();
        account.credit(10000.into());

        let mut dispute_account = Account::new();
        dispute_account.credit(10000.into());
        dispute_account.try_dispute(5000.into()).unwrap();

        let mut frozen_account = Account::new();
        frozen_account.credit(10000.into());
        frozen_account.try_dispute(5000.into()).unwrap();
        frozen_account.try_chargeback(5000.into()).unwrap();

        let mut accounts = HashMap::new();
        accounts.insert(1, account);
        accounts.insert(2, dispute_account);
        accounts.insert(3, frozen_account);

        let bank = Bank {
            accounts,
            transactions: HashMap::new(),
            disputes: HashMap::new(),
        };

        let mut accounts: Vec<(u16, Amount, Amount, Amount, bool)> = bank.accounts_iter().collect();
        accounts.sort_by(|x, y| x.0.cmp(&y.0));

        assert_eq!(
            accounts,
            vec![
                (1, 1.0.into(), 0.0.into(), 1.0.into(), false),
                (2, 0.5.into(), 0.5.into(), 1.0.into(), false),
                (3, 0.5.into(), 0.0.into(), 0.5.into(), true),
            ]
        )
    }
}
