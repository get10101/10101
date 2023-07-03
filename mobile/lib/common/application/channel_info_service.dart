import 'package:get_10101/common/domain/channel.dart';
import 'package:get_10101/ffi.dart' as rust;

import '../domain/model.dart';

class ChannelInfoService {
  const ChannelInfoService();

  Future<ChannelInfo?> getChannelInfo() async {
    rust.ChannelInfo? channelInfo = await rust.api.channelInfo();
    return channelInfo != null ? ChannelInfo.fromApi(channelInfo) : null;
  }

  Amount getMaxCapacity() {
    // This value is what we agree on as channel capacity cap for the beta
    return Amount(200000);
  }

  Amount getInitialReserve() {
    // This is the minimum value that has to remain in the channel.
    // It is defined by the transaction fees needed to close the channel (commit tx).
    // This fee is dynamically calculated when opening the channel, but for the beta we define a maximum of 20 sats/vbyte.
    // Given only one output a channel force close would require 3066 sats if we assume the maximum of 20 sats/vbyte.
    // For simplicity we hard-code the channel reserve to a slightly higher value to be on the safe side.
    return Amount(3100);
  }

  Amount getTradeFeeReserve() {
    // TODO: Fetch from backend
    // This hardcoded value corresponds to the fee-rate of 4 sats per vbyte. We should relate this value to that fee-rate in the backend.
    return Amount(1666);
  }

  Amount getMinTradeMargin() {
    // This value is an arbitrary number; we only allow trades with a minimum of 1000 sats margin.
    return Amount(1000);
  }
}
