import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;

import 'model.dart';

class ChannelInfo {
  final Amount channelCapacity;
  final Amount reserve;

  ChannelInfo(this.channelCapacity, this.reserve);

  static ChannelInfo fromApi(bridge.ChannelInfo channelInfo) {
    return ChannelInfo(Amount(channelInfo.channelCapacity),
        channelInfo.reserve != null ? Amount(channelInfo.reserve!) : Amount.zero());
  }
}
