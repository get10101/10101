import 'package:get_10101/common/domain/channel.dart';
import 'package:get_10101/ffi.dart' as rust;
import 'package:get_10101/common/domain/model.dart';

class ChannelInfoService {
  const ChannelInfoService();

  Future<ChannelInfo?> getChannelInfo() async {
    rust.ChannelInfo? channelInfo = await rust.api.channelInfo();
    return channelInfo != null ? ChannelInfo.fromApi(channelInfo) : null;
  }

  Future<Amount> getChannelOpenFeeEstimate() async {
    int feeEstimate = await rust.api.getChannelOpenFeeEstimateSat();
    return Amount(feeEstimate);
  }

  /// The multiplier that is used to determine the coordinator liquidity
  ///
  /// This value is an arbitrary number that may be subject to change.
  int getCoordinatorLiquidityMultiplier() {
    return rust.api.coordinatorLiquidityMultiplier();
  }

  /// The agreed upon maximum channel capacity for the beta
  ///
  /// This value is an arbitrary number that may be subject to change.
  Future<Amount> getMaxCapacity() async {
    int maxCapacity = await rust.api.maxChannelValue();
    return Amount(maxCapacity);
  }

  /// The assumed channel reserve if no channel was opened yet
  ///
  /// The channel reserve is defined by the transaction fees needed to close the channel (commit tx).
  /// Before we have an actual channel we assume the reserve to be 3066 sats which is maximum at 20 sats/vbyte.
  /// For simplicity we hard-code the initial channel reserve to a slightly higher value to be on the safe side.
  Amount getInitialReserve() {
    return Amount(3100);
  }

  Future<Amount> getTradeFeeReserve() async {
    double txFeesreserveForForceCloseAtOneSatsPerVbyte = 416.5;

    int satsPerVbyte = await rust.api.contractTxFeeRate();
    int feeReserve = (txFeesreserveForForceCloseAtOneSatsPerVbyte * satsPerVbyte).ceil();
    return Amount(feeReserve);
  }

  /// We only allow trades with a minimum of 1000 sats margin.
  ///
  /// This value is an arbitrary number that may be subject to change.
  Amount getMinTradeMargin() {
    return Amount(1000);
  }
}
