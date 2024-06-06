import 'package:get_10101/ffi.dart' as rust;

class ChannelInfoService {
  const ChannelInfoService();

  rust.TradeConstraints getTradeConstraints() {
    return rust.api.channelTradeConstraints();
  }

  double findCoordinatorLeverage(int traderLeverage) {
    var channelTradeConstraints = getTradeConstraints();
    try {
      return channelTradeConstraints.coordinatorLeverages
          .firstWhere((item) => item.traderLeverage == traderLeverage)
          .coordinatorLeverage
          .toDouble();
    } catch (e) {
      return channelTradeConstraints.defaultCoordinatorLeverage.toDouble();
    }
  }
}
