import 'package:get_10101/ffi.dart' as rust;

class ChannelInfoService {
  const ChannelInfoService();

  rust.TradeConstraints getTradeConstraints() {
    return rust.api.channelTradeConstraints();
  }
}
