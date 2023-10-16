import 'package:get_10101/bridge_generated/bridge_definitions.dart' as bridge;
import 'package:get_10101/common/domain/model.dart';

class ChannelInfo {
  final Amount channelCapacity;
  final Amount reserve;
  final int? liquidityOptionId;

  ChannelInfo(this.channelCapacity, this.reserve, this.liquidityOptionId);

  static ChannelInfo fromApi(bridge.ChannelInfo channelInfo) {
    return ChannelInfo(
        Amount(channelInfo.channelCapacity),
        channelInfo.reserve != null ? Amount(channelInfo.reserve!) : Amount.zero(),
        channelInfo.liquidityOptionId);
  }
}
