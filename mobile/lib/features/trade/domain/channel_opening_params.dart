import 'package:get_10101/common/domain/model.dart';

class ChannelOpeningParams {
  Amount coordinatorCollateral;
  Amount traderCollateral;

  ChannelOpeningParams({required this.coordinatorCollateral, required this.traderCollateral});
}
