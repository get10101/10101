import 'package:get_10101/common/domain/dlc_channel.dart';
import 'package:get_10101/ffi.dart' as rust;

class DlcChannelService {
  const DlcChannelService();

  Future<List<DlcChannel>> getDlcChannels() async {
    final apiDlcChannels = await rust.api.listDlcChannels();

    return apiDlcChannels.map((channel) => DlcChannel.fromApi(channel)).toList();
  }

  Future<void> deleteDlcChannel(String dlcChannelId) async {
    await rust.api.deleteDlcChannel(dlcChannelId: dlcChannelId);
  }
}
