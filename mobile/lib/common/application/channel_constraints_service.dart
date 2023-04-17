class ChannelConstraintsService {
  const ChannelConstraintsService();

  int getLightningChannelCapacity() {
    // This value is what we agree on as channel capacity cap for the beta
    return 200000;
  }

  int getChannelReserve() {
    // TODO: Fetch from backend
    // This is the minimum value that has to remain in the channel. It is defined in rust-lightning and we should fetch this value from the corresponding constant in the backend.
    return 1000;
  }

  int getFeeReserve() {
    // TODO: Fetch from backend
    // This hardcoded value corresponds to the fee-rate of 4 sats per vbyte. We should relate this value to that fee-rate in the backend.
    return 1666;
  }

  int getMinTradeMargin() {
    // This value is an arbitrary number; we only allow trades with a minimum of 1000 sats margin.
    return 1000;
  }
}
