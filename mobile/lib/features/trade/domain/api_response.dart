import 'package:get_10101/features/trade/domain/response_status.dart';

class ApiResponse {
  ResponseStatus status;
  String? errorText;

  ApiResponse({required this.status, this.errorText});
}
