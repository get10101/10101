class ChannelConstraintsService {
  const ChannelConstraintsService();

  int getLightningChannelCapacity() {
    // TODO: Fetch from backend
    // This value is what we agree on as channel capacity cap for the beta
    return 200000;
  }

  int getUsableChannelCapacity() {
    // To allow trading the channel has to stay balanced, so the usable capacity is 50% of the channel.
    // 50% is arbitrary - depending on the user's trading strategy it might be better to use less.
    return (getLightningChannelCapacity() / 2).floor();
  }

  int getChannelReserve() {
    // TODO: Fetch from backend
    // This is the minimum value that has to remain in the channel.
    // It is defined by the transaction fees needed to close the channel (commit tx).
    // This fee is dynamically calculated when opening the channel, but for the beta we define a maximum of 20 sats/vbyte.
    // Given only one output a channel force close would require 3066 sats if we assume the maximum of 20 sats/vbyte.
    // For simplicity we hard-code the channel reserve to a slightly higher value to be on the safe side.
    return 3100;
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
