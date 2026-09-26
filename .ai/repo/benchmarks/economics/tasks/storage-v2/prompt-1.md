Invoices can be in any supported currency, but the storage format does not record an
invoice's currency, so every invoice loads back as EUR. Fix this with a new version of the
storage format that records each invoice's currency. Existing files must keep loading.
