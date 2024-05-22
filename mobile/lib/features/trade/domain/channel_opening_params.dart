import 'package:get_10101/common/domain/model.dart';

class ChannelOpeningParams {
  Amount coordinatorReserve;
  Amount traderReserve;

  ChannelOpeningParams({required this.coordinatorReserve, required this.traderReserve});
}
